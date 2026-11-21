#[derive(Debug, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum InputEvent {
    DragBegin {
        gesture: u64,
        x: f64,
        y: f64,
        width: u32,
        height: u32,
    },
    DragMove {
        gesture: u64,
        dx: f64,
        dy: f64,
    },
    DragEnd {
        gesture: u64,
        dx: f64,
        dy: f64,
    },
    Move {
        x: f64,
        y: f64,
        width: u32,
        height: u32,
    },
    Button {
        x: f64,
        y: f64,
        width: u32,
        height: u32,
        button: u8,
        down: bool,
    },
    Wheel {
        delta: i32,
    },
    Key {
        vk: u16,
        down: bool,
    },
    Text {
        text: String,
    },
    Release,
}

/// A gesture is anchored once in the host desktop coordinate space. Later
/// window movement, resize and delayed geometry reports never rebase it.
#[derive(Clone, Copy, Debug)]
pub struct DragAnchor {
    pub gesture: u64,
    pub x: i32,
    pub y: i32,
}
impl DragAnchor {
    pub fn point(self, gesture: u64, dx: f64, dy: f64) -> Option<(i32, i32)> {
        if gesture == 0
            || gesture != self.gesture
            || !dx.is_finite()
            || !dy.is_finite()
            || dx.abs() > 131_072.0
            || dy.abs() > 131_072.0
        {
            return None;
        }
        Some((
            self.x.checked_add(dx.round() as i32)?,
            self.y.checked_add(dy.round() as i32)?,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cumulative_drag_never_accumulates_window_feedback_or_replayed_moves() {
        let anchor = DragAnchor {
            gesture: 7,
            x: -800,
            y: 250,
        };
        assert_eq!(anchor.point(7, 120.0, -30.0), Some((-680, 220)));
        assert_eq!(anchor.point(7, 120.0, -30.0), Some((-680, 220)));
        assert_eq!(anchor.point(7, 0.0, 0.0), Some((-800, 250)));
        assert_eq!(anchor.point(7, -25.0, 40.0), Some((-825, 290)));
        for (id, x, y) in [
            (0, 0.0, 0.0),
            (6, 0.0, 0.0),
            (7, f64::NAN, 0.0),
            (7, 0.0, f64::INFINITY),
            (7, 131073.0, 0.0),
        ] {
            assert_eq!(anchor.point(id, x, y), None);
        }
        assert_eq!(
            DragAnchor {
                gesture: 1,
                x: i32::MAX,
                y: 0
            }
            .point(1, 1.0, 0.0),
            None
        );
    }
}
