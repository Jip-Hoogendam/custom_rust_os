use alloc::fmt;
use core::cmp;
use noto_sans_mono_bitmap::{FontWeight, RasterHeight, get_raster, get_raster_width};

use crate::display::frame_buffer::{BGRColor, FrameBufferWriter};

pub struct Font {
    /// The height of the font
    pub raster_height: RasterHeight,
    /// The width of the font in pixels
    pub raster_width: usize,
    /// The font weight
    pub font_weight: FontWeight,
    /// The color of the font
    pub color: BGRColor,
    /// The color of the background
    pub bg_color: BGRColor,
}

impl Default for Font {
    fn default() -> Self {
        Self {
            raster_height: RasterHeight::Size16,
            raster_width: get_raster_width(FontWeight::Regular, RasterHeight::Size16),
            font_weight: FontWeight::Regular,
            color: BGRColor {
                blue: 255,
                green: 198,
                red: 255,
            },
            bg_color: BGRColor {
                blue: 49,
                green: 34,
                red: 32,
            },
        }
    }
}

/// Options to setup the tty with
pub struct TTYOptions {
    /// The refrenced fontmap
    pub font: Font,
    /// Padding around the outside of the display area
    pub padding: usize,
    /// The extra distance between letters
    pub kerning: usize,
    // The vertial space between each line
    pub vertical_space: usize,
    // The char used if we cannot lookup the given char
    pub fallback_char: char,
    /// the width of a tab.
    pub tab_width: u8,
}

impl Default for TTYOptions {
    fn default() -> Self {
        Self {
            font: Font::default(),
            padding: 10,
            kerning: 0,
            vertical_space: 2,
            fallback_char: '�',
            tab_width: 4,
        }
    }
}

pub struct TTYDisplay<T>
where
    T: FrameBufferWriter,
{
    /// The saved buffer k
    display_buffer: T,
    options: TTYOptions,
    cursor_x: usize,
    cursor_y: usize,
}

impl<T> TTYDisplay<T>
where
    T: FrameBufferWriter,
{
    pub const fn unint(display_buffer: T) -> Self {
        Self {
            display_buffer,
            options: TTYOptions {
                font: Font {
                    raster_height: RasterHeight::Size16,
                    raster_width: 0,
                    font_weight: FontWeight::Regular,
                    color: BGRColor {
                        blue: 0,
                        green: 0,
                        red: 0,
                    },
                    bg_color: BGRColor {
                        blue: 0,
                        green: 0,
                        red: 0,
                    },
                },
                padding: 0,
                kerning: 0,
                vertical_space: 0,
                fallback_char: ' ',
                tab_width: 0,
            },
            cursor_x: 0,
            cursor_y: 0,
        }
    }

    pub fn init(&mut self, display_buffer: T, options: TTYOptions) {
        self.display_buffer = display_buffer;
        self.cursor_x = options.padding;
        self.cursor_y = options.padding;
        self.options = options;
        self.clear();
    }

    pub fn clear(&mut self) {
        self.cursor_x = self.options.padding;
        self.cursor_y = self.options.padding;
        self.display_buffer.fill(&self.options.font.bg_color);
    }

    /// Creates a new writer.
    pub const fn new(display_buffer: T, options: TTYOptions) -> Self {
        let padding = options.padding;
        Self {
            display_buffer,
            options,
            cursor_x: padding,
            cursor_y: padding,
        }
    }

    /// Creates a new line.
    fn newline(&mut self) {
        let new_y =
            self.cursor_y + self.options.vertical_space + self.options.font.raster_height.val();
        self.cursor_x = self.options.padding;

        let diff = if new_y > self.display_buffer.height() as usize - self.options.padding {
            let new_new_y = self.display_buffer.height() as usize
                - self.options.padding
                - self.options.font.raster_height.val();
            new_y - new_new_y
        } else {
            self.cursor_y = new_y;
            return;
        };
        self.display_buffer.shift(diff);
    }

    /// Prints the background at a given location
    fn print_bg(&mut self) {
        let x1 = self.cursor_x;
        let y1 = self.cursor_y + self.options.font.raster_height.val();
        let x2 = self.cursor_x + self.options.font.raster_width;
        let y2 = self.cursor_y;
        self.display_buffer
            .write_rect((x1, y1), (x2, y2), &self.options.font.bg_color);
    }

    /// Displays a single char and advances the curser
    pub fn display_char(&mut self, c: char) {
        match c {
            '\n' => self.newline(),
            '\r' => self.cursor_x = self.options.padding,
            '\t' => {
                let char_index = self.cursor_x / self.options.font.raster_width;
                let tab_left = (char_index as isize) % self.options.tab_width as isize + 1;
                self.shift_cursor(tab_left);
            }
            '\u{80}' => {
                self.print_bg();
                self.shift_cursor(-1);
            }
            other => {
                self.print_char(other);
                self.shift_cursor(1);
            }
        }
    }

    /// Shifts a curser by a set amount.
    fn shift_cursor(&mut self, chars: isize) {
        let pixels =
            self.options.font.raster_width as isize * chars + self.options.kerning as isize;
        let new_pos = self.cursor_x as isize + pixels;

        if new_pos + self.options.font.raster_width as isize
            > self.display_buffer.width() as isize - self.options.padding as isize
        {
            self.newline();
        } else {
            self.cursor_x = cmp::max(new_pos, self.options.padding as isize) as usize;
        }
    }

    /// Prints a char in the provided frame buffer
    fn print_char(&mut self, c: char) {
        self.print_bg();
        let raster = if let Some(raster) = get_raster(
            c,
            self.options.font.font_weight,
            self.options.font.raster_height,
        ) {
            raster
        } else if let Some(raster) = get_raster(
            self.options.fallback_char,
            self.options.font.font_weight,
            self.options.font.raster_height,
        ) {
            raster
        } else {
            //There is nothing else to do!
            return;
        };
        for (y, row) in raster.raster().iter().enumerate() {
            for (x, byte) in row.iter().enumerate() {
                let pixel = self
                    .options
                    .font
                    .color
                    .mix(&self.options.font.bg_color, *byte);
                let x = self.cursor_x + x;
                let y = self.cursor_y + y;
                self.display_buffer.write_pixel(x, y, &pixel);
            }
        }
    }
}

unsafe impl<T> Send for TTYDisplay<T> where T: FrameBufferWriter {}
unsafe impl<T> Sync for TTYDisplay<T> where T: FrameBufferWriter {}

impl<T> fmt::Write for TTYDisplay<T>
where
    T: FrameBufferWriter,
{
    #[inline]
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for c in s.chars() {
            self.display_char(c);
        }
        Ok(())
    }
}
