//! Scrollbar geometry shared by painting and pointer interaction.
use super::*;

pub(super) const BAR_SIZE: f32 = 10.0;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Axis {
    Horizontal,
    Vertical,
}
impl Axis {
    pub fn coordinate(self, point: Point<Pixels>) -> Pixels {
        match self {
            Self::Horizontal => point.x,
            Self::Vertical => point.y,
        }
    }
    fn length(self, bounds: Bounds<Pixels>) -> Pixels {
        match self {
            Self::Horizontal => bounds.size.width,
            Self::Vertical => bounds.size.height,
        }
    }
}
#[derive(Clone)]
pub(super) struct Scrollbar {
    pub axis: Axis,
    pub track: Bounds<Pixels>,
    pub thumb: Bounds<Pixels>,
    pub maximum: Pixels,
    pub page: Pixels,
}
impl Scrollbar {
    pub fn new(
        axis: Axis,
        track: Bounds<Pixels>,
        page: Pixels,
        content: Pixels,
        offset: Pixels,
    ) -> Option<Self> {
        let length = axis.length(track);
        if content <= page || page <= px(0.0) || length <= px(0.0) {
            return None;
        }
        let maximum = content - page;
        let thumb_length = (length * (page / content)).max(px(24.0)).min(length * 0.8);
        let origin = (length - thumb_length) * (offset / maximum).clamp(0.0, 1.0);
        let thumb = match axis {
            Axis::Horizontal => Bounds::new(
                track.origin + point(origin, px(2.0)),
                size(thumb_length, px(BAR_SIZE - 4.0)),
            ),
            Axis::Vertical => Bounds::new(
                track.origin + point(px(2.0), origin),
                size(px(BAR_SIZE - 4.0), thumb_length),
            ),
        };
        Some(Self {
            axis,
            track,
            thumb,
            maximum,
            page,
        })
    }
    pub fn grab_fraction(&self, position: Point<Pixels>) -> f32 {
        ((self.axis.coordinate(position) - self.axis.coordinate(self.thumb.origin))
            / self.axis.length(self.thumb))
        .clamp(0.0, 1.0)
    }
    pub fn offset_for_drag(&self, position: Point<Pixels>, grab: f32) -> Pixels {
        let travel = self.axis.length(self.track) - self.axis.length(self.thumb);
        let coordinate = self.axis.coordinate(position)
            - self.axis.coordinate(self.track.origin)
            - self.axis.length(self.thumb) * grab;
        self.maximum * (coordinate / travel).clamp(0.0, 1.0)
    }
}
