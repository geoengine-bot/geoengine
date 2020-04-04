use crate::error;
use crate::operations::image::{Colorizer, LossyInto, RgbaTransmutable};
use crate::raster::{GridPixelAccess, Raster, Raster2D};
use crate::util::Result;
use image::{DynamicImage, ImageFormat, RgbaImage};

pub trait ToPng {
    /// Outputs png bytes of an image of size width x height
    fn to_png(&self, width: u32, height: u32, colorizer: &Colorizer) -> Result<Vec<u8>>;
}

impl<T> ToPng for Raster2D<T>
where
    T: Copy + LossyInto<f64> + RgbaTransmutable,
{
    fn to_png(&self, width: u32, height: u32, colorizer: &Colorizer) -> Result<Vec<u8>> {
        // TODO: use PNG color palette once it is available

        let scale_x = (self.dimension()[0] as f64) / f64::from(width);
        let scale_y = (self.dimension()[1] as f64) / f64::from(height);

        let color_mapper = colorizer.create_color_mapper();

        let image_buffer: RgbaImage = RgbaImage::from_fn(width, height, |x, y| {
            if let Ok(pixel_value) =
                self.pixel_value_at_grid_index(&image_pixel_to_raster_pixel(x, y, scale_x, scale_y))
            {
                color_mapper.call(pixel_value)
            } else {
                colorizer.no_data_color()
            }
            .into()
        });

        let mut buffer = Vec::new();

        DynamicImage::ImageRgba8(image_buffer)
            .write_to(&mut buffer, ImageFormat::Png)
            .map_err(|_| error::Error::Colorizer {
                details: "encoding PNG failed".into(),
            })?;

        Ok(buffer)
    }
}

// TODO: move these functions to base raster (?)
/// Map an image's (x, y) values to the grid cells of a raster.
fn image_pixel_to_raster_pixel<ImagePixelType>(
    x: ImagePixelType,
    y: ImagePixelType,
    scale_x: f64,
    scale_y: f64,
) -> (usize, usize)
where
    ImagePixelType: Into<f64>,
{
    debug_assert!(
        scale_x > 0. && scale_y > 0.,
        "scale values must be positive"
    );

    let cell_x = (((x.into() + 0.5) * scale_x) - 0.5).round();
    let cell_y = (((y.into() + 0.5) * scale_y) - 0.5).round();
    (cell_x as usize, cell_y as usize)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operations::image::RgbaColor;
    use crate::raster::GridPixelAccessMut;

    #[test]
    fn linear_gradient() {
        let mut raster = Raster2D::new(
            [2, 2].into(),
            vec![0; 4],
            None,
            Default::default(),
            Default::default(),
        )
        .unwrap();

        raster.set_pixel_value_at_grid_index(&(0, 0), 255).unwrap();
        raster.set_pixel_value_at_grid_index(&(0, 1), 100).unwrap();

        let colorizer = Colorizer::linear_gradient(
            vec![
                (0.0.into(), RgbaColor::new(0, 0, 0, 255)).into(),
                (255.0.into(), RgbaColor::new(255, 255, 255, 255)).into(),
            ],
            RgbaColor::transparent(),
            RgbaColor::pink(),
        )
        .unwrap();

        let image_bytes = raster.to_png(100, 100, &colorizer).unwrap();

        assert_eq!(
            include_bytes!("../../../test-data/colorizer/linear_gradient.png") as &[u8],
            image_bytes.as_slice()
        );
    }

    #[test]
    fn logarithmic_gradient() {
        let mut raster = Raster2D::new(
            [2, 2].into(),
            vec![1; 4],
            None,
            Default::default(),
            Default::default(),
        )
        .unwrap();

        raster.set_pixel_value_at_grid_index(&(0, 0), 10).unwrap();
        raster.set_pixel_value_at_grid_index(&(0, 1), 5).unwrap();

        let colorizer = Colorizer::logarithmic_gradient(
            vec![
                (1.0.into(), RgbaColor::new(0, 0, 0, 255)).into(),
                (10.0.into(), RgbaColor::new(255, 255, 255, 255)).into(),
            ],
            RgbaColor::transparent(),
            RgbaColor::pink(),
        )
        .unwrap();

        let image_bytes = raster.to_png(100, 100, &colorizer).unwrap();

        assert_eq!(
            include_bytes!("../../../test-data/colorizer/logarithmic_gradient.png") as &[u8],
            image_bytes.as_slice()
        );
    }

    #[test]
    fn palette() {
        let mut raster = Raster2D::new(
            [2, 2].into(),
            vec![0; 4],
            None,
            Default::default(),
            Default::default(),
        )
        .unwrap();

        raster.set_pixel_value_at_grid_index(&(0, 0), 2).unwrap();
        raster.set_pixel_value_at_grid_index(&(0, 1), 1).unwrap();

        let colorizer = Colorizer::palette(
            [
                (0.0.into(), RgbaColor::new(0, 0, 0, 255)),
                (1.0.into(), RgbaColor::new(255, 0, 0, 255)),
                (2.0.into(), RgbaColor::new(255, 255, 255, 255)),
            ]
            .iter()
            .cloned()
            .collect(),
            RgbaColor::transparent(),
        )
        .unwrap();

        let image_bytes = raster.to_png(100, 100, &colorizer).unwrap();

        assert_eq!(
            include_bytes!("../../../test-data/colorizer/palette.png") as &[u8],
            image_bytes.as_slice()
        );
    }

    #[test]
    fn rgba() {
        let mut raster = Raster2D::new(
            [2, 2].into(),
            vec![0x0000_00FF_u32; 4],
            None,
            Default::default(),
            Default::default(),
        )
        .unwrap();

        raster
            .set_pixel_value_at_grid_index(&(0, 0), 0xFF00_00FF_u32)
            .unwrap();
        raster
            .set_pixel_value_at_grid_index(&(0, 1), 0x00FF_00FF_u32)
            .unwrap();

        let colorizer = Colorizer::rgba();

        let image_bytes = raster.to_png(100, 100, &colorizer).unwrap();

        assert_eq!(
            include_bytes!("../../../test-data/colorizer/rgba.png") as &[u8],
            image_bytes.as_slice()
        );
    }
}