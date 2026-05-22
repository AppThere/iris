// Copyright 2024 AppThere Project
// SPDX-License-Identifier: Apache-2.0

use std::io::Cursor;
use super::importer::{detect_format, ImageFormat};
use super::import_raster_image;
use iris_pixel::{BitDepth, ChannelLayout, LayerContent, TileCoord};

fn make_dummy_image(width: u32, height: u32, format: image::ImageFormat) -> Vec<u8> {
    let mut buf = image::ImageBuffer::new(width, height);
    for (x, y, pixel) in buf.enumerate_pixels_mut() {
        *pixel = image::Rgba([
            ((x * 255) / width) as u8,
            ((y * 255) / height) as u8,
            128,
            255,
        ]);
    }
    let img = image::DynamicImage::ImageRgba8(buf);
    let mut cursor = Cursor::new(Vec::new());
    if format == image::ImageFormat::Jpeg {
        img.to_rgb8().write_to(&mut cursor, format).unwrap();
    } else {
        img.write_to(&mut cursor, format).unwrap();
    }
    cursor.into_inner()
}

fn write_image_exr(width: usize, height: usize) -> Vec<u8> {
    use exr::prelude::*;
    let get_pixel = |Vec2(x, y): Vec2<usize>| {
        let val = f16::from_f32((x + y) as f32 / 10.0);
        (val, val, val, f16::ONE)
    };
    let layer = Layer::new(
        (width, height),
        LayerAttributes::named(Text::from("rgba")),
        Encoding::FAST_LOSSLESS,
        SpecificChannels::rgba(get_pixel),
    );
    let image = Image::from_layer(layer);
    let mut buf = Cursor::new(Vec::new());
    image.write().to_buffered(&mut buf).unwrap();
    buf.into_inner()
}

#[test]
fn test_detect_format() {
    let png = make_dummy_image(2, 2, image::ImageFormat::Png);
    assert_eq!(detect_format(&png), Some(ImageFormat::Png));

    let jpeg = make_dummy_image(2, 2, image::ImageFormat::Jpeg);
    assert_eq!(detect_format(&jpeg), Some(ImageFormat::Jpeg));

    let webp = make_dummy_image(2, 2, image::ImageFormat::WebP);
    assert_eq!(detect_format(&webp), Some(ImageFormat::Webp));

    let exr = write_image_exr(2, 2);
    assert_eq!(detect_format(&exr), Some(ImageFormat::Exr));

    let invalid = vec![1, 2, 3, 4];
    assert_eq!(detect_format(&invalid), None);
}

#[test]
fn test_import_png() {
    let png = make_dummy_image(300, 200, image::ImageFormat::Png);
    let layer = import_raster_image(&png, "Test PNG").unwrap();
    assert_eq!(layer.name, "Test PNG");
    assert!(layer.visible);
    if let LayerContent::Pixel(pixel_layer) = layer.content {
        assert_eq!(pixel_layer.bit_depth, BitDepth::F16);
        assert_eq!(pixel_layer.channel_layout, ChannelLayout::Rgba);
        if let Some(bounds) = pixel_layer.crop_bounds {
            assert_eq!(bounds.width, 300);
            assert_eq!(bounds.height, 200);
        } else {
            panic!("Expected crop_bounds");
        }
        assert_eq!(pixel_layer.tiles.get(TileCoord { tx: 0, ty: 0 }).is_some(), true);
        assert_eq!(pixel_layer.tiles.get(TileCoord { tx: 1, ty: 0 }).is_some(), true);
        assert_eq!(pixel_layer.tiles.get(TileCoord { tx: 0, ty: 1 }).is_some(), false);
    } else {
        panic!("Expected Pixel layer content");
    }
}

#[test]
fn test_import_jpeg() {
    let jpeg = make_dummy_image(10, 10, image::ImageFormat::Jpeg);
    let layer = import_raster_image(&jpeg, "Test JPEG").unwrap();
    assert_eq!(layer.name, "Test JPEG");
    if let LayerContent::Pixel(pixel_layer) = layer.content {
        assert_eq!(pixel_layer.crop_bounds.unwrap().width, 10);
        assert_eq!(pixel_layer.tiles.get(TileCoord { tx: 0, ty: 0 }).is_some(), true);
    } else {
        panic!("Expected Pixel layer content");
    }
}

#[test]
fn test_import_webp() {
    let webp = make_dummy_image(10, 10, image::ImageFormat::WebP);
    let layer = import_raster_image(&webp, "Test WebP").unwrap();
    assert_eq!(layer.name, "Test WebP");
    if let LayerContent::Pixel(pixel_layer) = layer.content {
        assert_eq!(pixel_layer.crop_bounds.unwrap().width, 10);
        assert_eq!(pixel_layer.tiles.get(TileCoord { tx: 0, ty: 0 }).is_some(), true);
    } else {
        panic!("Expected Pixel layer content");
    }
}

#[test]
fn test_import_exr() {
    let exr = write_image_exr(10, 10);
    let layer = import_raster_image(&exr, "Test EXR").unwrap();
    assert_eq!(layer.name, "Test EXR");
    if let LayerContent::Pixel(pixel_layer) = layer.content {
        assert_eq!(pixel_layer.crop_bounds.unwrap().width, 10);
        assert_eq!(pixel_layer.tiles.get(TileCoord { tx: 0, ty: 0 }).is_some(), true);
    } else {
        panic!("Expected Pixel layer content");
    }
}

#[test]
fn test_import_invalid() {
    let err = import_raster_image(&[0, 1, 2, 3], "Invalid").unwrap_err();
    assert!(matches!(err, crate::error::AifError::ImportError(_)));
}
