use core::{mem, ops::Div};

use bootloader_api::info::FrameBuffer;


/// A pixel in the BGR format type
/// Used to write to frame buffers
#[derive(Clone)]
pub struct BGRColor {
    pub blue: u8,
    pub green: u8,
    pub red: u8,
}

impl BGRColor {
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn mix(&self, other: &Self, amount: u8) -> Self {
        let amount = u16::from(amount);
        let inverse = 255 - amount;

        Self {
            blue: ((u16::from(self.blue) * amount + u16::from(other.blue) * inverse) / 255) as u8,
            green: ((u16::from(self.green) * amount + u16::from(other.green) * inverse) / 255)
                as u8,
            red: ((u16::from(self.red) * amount + u16::from(other.red) * inverse) / 255) as u8,
        }
    }
}

impl Div<&u8> for BGRColor {
    type Output = Self;
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::cast_sign_loss)]
    fn div(self, rhs: &u8) -> Self::Output {
        let div = f32::from(*rhs).div(255.);
        Self {
            red: f32::from(self.red).div(div) as u8,
            blue: f32::from(self.blue).div(div) as u8,
            green: f32::from(self.green).div(div) as u8,
        }
    }
}

pub trait FrameBufferWriter {
    /// Writes to an exact pixel.
    fn write_pixel(&mut self, x: usize, y: usize, pixel: &BGRColor);
    /// Reads an exact pixel
    fn read_pixel(&self, x: usize, y: usize) -> BGRColor;

    /// Gets the witdth of the display in pixels
    fn width(&self) -> u32;

    ///Gets the width of the display in pixels
    fn height(&self) -> u32;

    /// Shifts the entire display by x pixels
    fn shift(&mut self, pixels: usize);

    fn fill(&mut self, pixels: &BGRColor);

    ///Writes a rectangle. This has a default implementation
    ///But it may be intresting to provide faster implemntation using SMID
    fn write_rect(
        &mut self,
        mut coord_1: (usize, usize),
        mut coord_2: (usize, usize),
        pixel: &BGRColor,
    ) {
        let width = self.width() as usize;
        let height = self.height() as usize;

        if coord_2.0 > coord_1.0 {
            mem::swap(&mut coord_1.0, &mut coord_2.0);
        }
        if coord_2.1 > coord_1.1 {
            mem::swap(&mut coord_1.1, &mut coord_2.1);
        }

        //clamp the values
        if coord_1.0 > width {
            coord_1.0 = width;
        }
        if coord_2.0 > width {
            coord_2.0 = width;
        }
        if coord_1.1 > height {
            coord_1.1 = height;
        }
        if coord_2.1 > height {
            coord_2.1 = height;
        }

        for x in coord_1.0..coord_2.0 {
            for y in coord_1.1..coord_2.1 {
                self.write_pixel(x, y, pixel);
            }
        }
    }
}

enum PixelFormat {
    Rgb,
    Bgr,
    U8,
}

pub struct DisplayBuffer<'a> {
    frame_buffer: &'a mut [u8],
    stride: u32,
    fmt: PixelFormat,
    pixel_size: u8,
}

impl DisplayBuffer<'_> {
    #[must_use]
    pub const fn uninit() -> Self {
        Self {
            frame_buffer: unsafe { core::slice::from_raw_parts_mut(core::ptr::dangling_mut(), 0) },
            stride: 0,
            fmt: PixelFormat::U8,
            pixel_size: 0,
        }
    }
}

impl<'a> From<&'a mut FrameBuffer> for DisplayBuffer<'a> {
    fn from(value: &'a mut FrameBuffer) -> Self {
        let info = value.info();
        #[allow(clippy::cast_possible_truncation)]
        Self {
            frame_buffer: value.buffer_mut(),
            stride: info.stride as u32,
            fmt: match info.pixel_format {
                bootloader_api::info::PixelFormat::U8 => PixelFormat::U8,
                bootloader_api::info::PixelFormat::Rgb => PixelFormat::Rgb,
                bootloader_api::info::PixelFormat::Bgr | _ => PixelFormat::Bgr,
            },
            pixel_size: info.bytes_per_pixel as u8,
        }
    }
}

impl FrameBufferWriter for DisplayBuffer<'_> {
    fn read_pixel(&self, x: usize, y: usize) -> BGRColor {
        let start_index = (y * self.stride as usize + x) * self.pixel_size as usize;
        let pixel = &self.frame_buffer[start_index..(start_index + self.pixel_size as usize)];
        match self.fmt {
            PixelFormat::Rgb => BGRColor {
                blue: pixel[2],
                green: pixel[1],
                red: pixel[0],
            },
            PixelFormat::Bgr => BGRColor {
                blue: pixel[0],
                green: pixel[1],
                red: pixel[2],
            },
            PixelFormat::U8 => BGRColor {
                blue: pixel[0],
                green: pixel[0],
                red: pixel[0],
            },
        }
    }

    fn fill(&mut self, pixel: &BGRColor) {
        match self.fmt {
            PixelFormat::Bgr => {
                //An optimized appproch to creating filling the frame by manipulating the fact that
                //the size is known.
                if self.pixel_size == 4 {
                    let (_, buffer, _) = unsafe { self.frame_buffer.align_to_mut() };
                    let bgr_fmt = u32::from(pixel.blue)
                        | u32::from(pixel.green) << 8
                        | u32::from(pixel.red) << 16;
                    buffer.fill(bgr_fmt);
                } else {
                    for range in self.frame_buffer.chunks_exact_mut(self.pixel_size as usize) {
                        if let [blue, green, red, ..] = range {
                            *blue = pixel.blue;
                            *green = pixel.green;
                            *red = pixel.red;
                        }
                    }
                }
            }
            PixelFormat::Rgb => {
                for range in self.frame_buffer.chunks_exact_mut(self.pixel_size as usize) {
                    if let [red, green, blue, ..] = range {
                        *blue = pixel.blue;
                        *green = pixel.green;
                        *red = pixel.red;
                    }
                }
            }
            PixelFormat::U8 => {
                let grayscale =
                    (u16::from(pixel.red) + u16::from(pixel.blue) + u16::from(pixel.green)) / 255;
                #[allow(clippy::cast_possible_truncation)]
                self.frame_buffer.fill(grayscale as u8);
            }
        }
    }

    fn shift(&mut self, pixels: usize) {
        let start = self.stride as usize * self.pixel_size as usize * pixels;
        self.frame_buffer.copy_within(start.., 0);
    }

    fn write_pixel(&mut self, x: usize, y: usize, pixel: &BGRColor) {
        let start_index = (y * self.stride as usize + x) * self.pixel_size as usize;
        let data = &mut self.frame_buffer[start_index..(start_index + self.pixel_size as usize)];
        match self.fmt {
            PixelFormat::Bgr => {
                data[0] = pixel.blue;
                data[1] = pixel.green;
                data[2] = pixel.red;
            }
            PixelFormat::Rgb => {
                data[0] = pixel.red;
                data[1] = pixel.green;
                data[2] = pixel.blue;
            }
            PixelFormat::U8 => data[0] = (pixel.red + pixel.blue + pixel.green).div_euclid(3),
        }
    }

    fn width(&self) -> u32 {
        self.stride
    }

    #[allow(clippy::cast_possible_truncation)]
    fn height(&self) -> u32 {
        (self.frame_buffer.len() / self.pixel_size as usize / self.stride as usize) as u32
    }
}
