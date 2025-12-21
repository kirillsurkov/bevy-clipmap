from numba import jit, prange
from joblib import Parallel, delayed
import numpy as np
import pyktx
import cv2
import pathlib
import argparse
from PIL import Image
Image.MAX_IMAGE_PIXELS = 933120000


@jit(parallel=True)
def stack(list_of_array):
    shape = (len(list_of_array),) + list_of_array[0].shape
    stacked_array = np.empty(shape, dtype=list_of_array[0].dtype)
    for j in prange(len(list_of_array)):
        stacked_array[j] = list_of_array[j]
    return stacked_array


@jit
def horizon_row_tangents(row):
    width = row.shape[0]
    horizons = np.arange(width)
    for j in range(width - 2, -1, -1):
        k = j + 1
        while True:
            nk = horizons[k]
            slope_k = max(0, (float(row[k]) - float(row[j])) / (k - j))
            slope_nk = max(
                0, (float(row[nk]) - float(row[j])) / (nk - j)) if nk != k else 0.0
            if slope_k > slope_nk:
                horizons[j] = k
                break
            if nk == k:
                break
            k = nk
    dx = horizons - np.arange(width)
    dy = row[horizons] - row[np.arange(width)]
    tangents = dy / dx
    tangents[np.isnan(tangents)] = 0.0
    return tangents


@jit
def horizon_map_tangents(heightmap):
    height = heightmap.shape[0]
    res = stack([horizon_row_tangents(heightmap[i]) for i in prange(height)])
    return res


def rotate_opencv(heightmap, angle_deg, order=1, reshape=True):
    rows, cols = heightmap.shape
    M = cv2.getRotationMatrix2D((cols / 2, rows / 2), angle_deg, 1.0)
    if reshape:
        theta = np.radians(angle_deg)
        abs_cos, abs_sin = abs(np.cos(theta)), abs(np.sin(theta))
        new_rows = int(np.ceil(rows * abs_cos + cols * abs_sin))
        new_cols = int(np.ceil(rows * abs_sin + cols * abs_cos))
        if (new_rows - rows) % 2 == 1:
            new_rows += 1
        if (new_cols - cols) % 2 == 1:
            new_cols += 1
        M[0, 2] += (new_cols / 2) - cols / 2
        M[1, 2] += (new_rows / 2) - rows / 2
        dsize = (new_cols, new_rows)
    else:
        dsize = (cols, rows)
    interp_flags = {
        0: cv2.INTER_NEAREST,
        1: cv2.INTER_LINEAR,
        3: cv2.INTER_CUBIC
    }.get(order, cv2.INTER_LINEAR)
    return cv2.warpAffine(
        heightmap, M, dsize,
        flags=interp_flags,
        borderMode=cv2.BORDER_REPLICATE,
        borderValue=0
    )


def horizon_map_tangents_azimuth(heightmap, angle, dest):
    horizon = rotate_opencv(heightmap, angle, order=1, reshape=True)
    horizon = horizon_map_tangents(horizon)
    # print(horizon.dtype)
    horizon = rotate_opencv(horizon, -angle, order=0, reshape=False)
    crop_y = (horizon.shape[0] - heightmap.shape[0]) // 2
    crop_x = (horizon.shape[1] - heightmap.shape[1]) // 2
    horizon = horizon[crop_y:crop_y+heightmap.shape[0],
                      crop_x:crop_x+heightmap.shape[1]]
    print(f'{dest}/horizon_{angle}.npy')
    np.save(f'{dest}/horizon_{angle}', horizon)


@jit
def fft_compress_row(n_coeffs, row):
    coeffs = np.fft.rfft(row, axis=0)
    k = n_coeffs // 2
    res = np.zeros((row.shape[1], n_coeffs+1))
    res[:, :k+1] = coeffs[0:k+1].real.T
    res[:, k+1:] = coeffs[1:k+1].imag.T
    return res


@jit(parallel=True)
def fft_compress(horizons, n_coeffs):
    height, width = horizons[0].shape
    res = np.zeros((height, width, n_coeffs+1), dtype=np.float32)
    for y in prange(height):
        row = stack([horizon[y] for horizon in horizons])
        res[y] = fft_compress_row(n_coeffs, row)
    return res


if __name__ == "__main__":
    def cmd_ktx(args):
        print(f"Convert {args.filename} to KTX2 {args.width}x{args.height}")

        heightmap = np.array(Image.open(args.filename).resize(
            (args.width, args.height)), dtype=np.float32).astype(np.uint16)

        texture = pyktx.KtxTexture2.create(pyktx.KtxTextureCreateInfo(
            gl_internal_format=None,
            base_width=heightmap.shape[1],
            base_height=heightmap.shape[0],
            base_depth=1,
            num_dimensions=2,
            num_levels=1,
            num_layers=1,
            num_faces=1,
            is_array=False,
            vk_format=pyktx.VkFormat.VK_FORMAT_R16_UNORM,
            generate_mipmaps=False,
        ), pyktx.KtxTextureCreateStorage.ALLOC)
        texture.set_image_from_memory(
            level=0,
            layer=0,
            face_slice=0,
            data=heightmap.tobytes('C'),
        )
        horizon_filename = '.'.join(args.filename.split('.')[:-1])
        horizon_filename += f'_{args.width}x{args.height}.ktx2'
        texture.write_to_named_file(horizon_filename)
        print('Done.')

    def cmd_horizon(args):
        print(f"Horizon map {args.width}x{args.height}, coeffs={args.coeffs}")

        azimuths = 360
        dest = './horizons'

        pathlib.Path(dest).mkdir(parents=True, exist_ok=True)

        heightmap = np.array(Image.open(args.filename).resize(
            (args.width, args.height)), dtype=np.float32) / 65535.0

        print('Computing horizon maps...')
        Parallel(8)(delayed(horizon_map_tangents_azimuth)(
            heightmap, angle, dest) for angle in range(azimuths))
        print('Done.')

        print('Compressing...')
        horizonmap = fft_compress([np.load(
            f'{dest}/horizon_{angle}.npy', mmap_mode='r') for angle in range(azimuths)], args.coeffs)
        print('Done.')

        print('Saving ktx2...')
        texture = pyktx.KtxTexture2.create(pyktx.KtxTextureCreateInfo(
            gl_internal_format=None,
            base_width=horizonmap.shape[1],
            base_height=horizonmap.shape[0],
            base_depth=1,
            num_dimensions=2,
            num_levels=1,
            num_layers=horizonmap.shape[2],
            num_faces=1,
            is_array=True,
            vk_format=pyktx.VkFormat.VK_FORMAT_R32_SFLOAT,
            generate_mipmaps=False,
        ), pyktx.KtxTextureCreateStorage.ALLOC)
        for layer in range(horizonmap.shape[2]):
            texture.set_image_from_memory(
                level=0,
                layer=layer,
                face_slice=0,
                data=horizonmap[:, :, layer].tobytes('C'),
            )
        horizon_filename = '.'.join(args.filename.split('.')[:-1])
        horizon_filename += f'_horizon_{args.width}x{args.height}_{args.coeffs}.ktx2'
        texture.write_to_named_file(horizon_filename)
        print('Done.')

    parser = argparse.ArgumentParser(
        description='Heightmap processing tool for the bevy-clipmap plugin')
    parser.add_argument('filename', help='16-bit PNG heightmap')

    subparsers = parser.add_subparsers(dest='command', required=True)

    p_ktx = subparsers.add_parser('ktx', help='Convert the heightmap to KTX2')
    p_ktx.add_argument('width', type=int, help='Output width')
    p_ktx.add_argument('height', type=int, help='Output height')
    p_ktx.set_defaults(func=cmd_ktx)

    p_horizon = subparsers.add_parser(
        'horizon', help='Create KTX2 horizon map')
    p_horizon.add_argument('width', type=int, help='Output width')
    p_horizon.add_argument('height', type=int, help='Output height')
    p_horizon.add_argument('coeffs', type=int, help='Number of FFT coeffs')
    p_horizon.set_defaults(func=cmd_horizon)

    args = parser.parse_args()
    args.func(args)
