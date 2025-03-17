use image::Luma;

#[derive(Copy, Clone)]
pub(crate) struct Bgru8([u8; 3]);

impl image::Pixel for Bgru8 {
    type Subpixel = u8;
    const CHANNEL_COUNT: u8 = 3;
    fn channels(&self) -> &[Self::Subpixel] {
        &self.0
    }

    fn channels_mut(&mut self) -> &mut [Self::Subpixel] {
        &mut self.0
    }

    const COLOR_MODEL: &'static str = "BGR";

    fn channels4(
        &self,
    ) -> (
        Self::Subpixel,
        Self::Subpixel,
        Self::Subpixel,
        Self::Subpixel,
    ) {
        (self.0[0], self.0[1], self.0[2], 0)
    }

    fn from_channels(
        a: Self::Subpixel,
        b: Self::Subpixel,
        c: Self::Subpixel,
        _d: Self::Subpixel,
    ) -> Self {
        Bgru8([a, b, c])
    }

    fn from_slice(slice: &[Self::Subpixel]) -> &Self {
        assert_eq!(slice.len(), 3);
        unsafe { &*(slice.as_ptr() as *const Bgru8) }
    }

    fn from_slice_mut(slice: &mut [Self::Subpixel]) -> &mut Self {
        assert_eq!(slice.len(), 3);
        unsafe { &mut *(slice.as_mut_ptr() as *mut Bgru8) }
    }

    fn to_rgb(&self) -> image::Rgb<Self::Subpixel> {
        image::Rgb([self.0[2], self.0[1], self.0[0]])
    }

    fn to_rgba(&self) -> image::Rgba<Self::Subpixel> {
        image::Rgba([self.0[2], self.0[1], self.0[0], 255])
    }

    fn to_luma(&self) -> Luma<Self::Subpixel> {
        let luma =
            (0.299 * self.0[2] as f32 + 0.587 * self.0[1] as f32 + 0.114 * self.0[0] as f32) as u8;
        image::Luma([luma])
    }

    fn to_luma_alpha(&self) -> image::LumaA<Self::Subpixel> {
        let luma =
            (0.299 * self.0[2] as f32 + 0.587 * self.0[1] as f32 + 0.114 * self.0[0] as f32) as u8;
        image::LumaA([luma, 255])
    }

    fn map<F>(&self, mut f: F) -> Self
    where
        F: FnMut(Self::Subpixel) -> Self::Subpixel,
    {
        Bgru8([f(self.0[0]), f(self.0[1]), f(self.0[2])])
    }

    fn apply<F>(&mut self, mut f: F)
    where
        F: FnMut(Self::Subpixel) -> Self::Subpixel,
    {
        self.0[0] = f(self.0[0]);
        self.0[1] = f(self.0[1]);
        self.0[2] = f(self.0[2]);
    }

    fn map_with_alpha<F, G>(&self, mut f: F, _g: G) -> Self
    where
        F: FnMut(Self::Subpixel) -> Self::Subpixel,
        G: FnMut(Self::Subpixel) -> Self::Subpixel,
    {
        Bgru8([f(self.0[0]), f(self.0[1]), f(self.0[2])])
    }

    fn apply_with_alpha<F, G>(&mut self, mut f: F, _g: G)
    where
        F: FnMut(Self::Subpixel) -> Self::Subpixel,
        G: FnMut(Self::Subpixel) -> Self::Subpixel,
    {
        self.0[0] = f(self.0[0]);
        self.0[1] = f(self.0[1]);
        self.0[2] = f(self.0[2]);
    }

    fn map2<F>(&self, other: &Self, mut f: F) -> Self
    where
        F: FnMut(Self::Subpixel, Self::Subpixel) -> Self::Subpixel,
    {
        Bgru8([
            f(self.0[0], other.0[0]),
            f(self.0[1], other.0[1]),
            f(self.0[2], other.0[2]),
        ])
    }

    fn apply2<F>(&mut self, other: &Self, mut f: F)
    where
        F: FnMut(Self::Subpixel, Self::Subpixel) -> Self::Subpixel,
    {
        self.0[0] = f(self.0[0], other.0[0]);
        self.0[1] = f(self.0[1], other.0[1]);
        self.0[2] = f(self.0[2], other.0[2]);
    }

    fn invert(&mut self) {
        self.0[0] = 255 - self.0[0];
        self.0[1] = 255 - self.0[1];
        self.0[2] = 255 - self.0[2];
    }

    fn blend(&mut self, other: &Self) {
        self.0[0] = (self.0[0] as u16 + other.0[0] as u16 / 2) as u8;
        self.0[1] = (self.0[1] as u16 + other.0[1] as u16 / 2) as u8;
        self.0[2] = (self.0[2] as u16 + other.0[2] as u16 / 2) as u8;
    }
}

pub(crate) type BgrImage = image::ImageBuffer<Bgru8, Vec<u8>>;

fn to_bgru8<T: image::Pixel<Subpixel = u8>, C: std::ops::Deref<Target = [T::Subpixel]>>(
    image: &image::ImageBuffer<T, C>,
) -> BgrImage {
    let mut out: BgrImage = BgrImage::new(image.width(), image.height());
    for (o, i) in out.pixels_mut().zip(image.pixels()) {
        let rgb = i.to_rgb();
        *o = Bgru8([rgb[2], rgb[1], rgb[0]]);
    }
    out
}

pub(crate) fn dyn_to_bgr(img: &image::DynamicImage) -> BgrImage {
    match img {
        image::DynamicImage::ImageLuma8(image_buffer) => to_bgru8(image_buffer),
        image::DynamicImage::ImageLumaA8(image_buffer) => to_bgru8(image_buffer),
        image::DynamicImage::ImageRgb8(image_buffer) => to_bgru8(image_buffer),
        image::DynamicImage::ImageRgba8(image_buffer) => to_bgru8(image_buffer),
        image::DynamicImage::ImageLuma16(_) => unimplemented!(),
        image::DynamicImage::ImageLumaA16(_) => unimplemented!(),
        image::DynamicImage::ImageRgb16(_) => unimplemented!(),
        image::DynamicImage::ImageRgba16(_) => unimplemented!(),
        image::DynamicImage::ImageRgb32F(_) => unimplemented!(),
        image::DynamicImage::ImageRgba32F(_) => unimplemented!(),
        _ => unimplemented!(),
    }
}
