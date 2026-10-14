//! Moving speed and altitude tapes and the vertical speed scale.
//!
//! A tape shows a value by position and motion, and the boxed value at its
//! index gives the exact reading. A failed or missing value stops the tape.
use super::*;

/// The scale of one tape.
pub(super) struct Tape<'a> {
    /// Quantity name drawn above the tape.
    pub label: &'a str,
    pub signal: Sig<f32>,
    pub group: GroupId,
    /// The x of the index line, next to the conformal field.
    pub index: f32,
    /// -1 when the tape extends left of its index, +1 when it extends right.
    pub side: f32,
    /// Value between ticks and between labels.
    pub minor: f32,
    pub major: f32,
    /// Value from the index to either end of the tape.
    pub span: f32,
    /// No mark is drawn below this value.
    pub floor: Option<f32>,
}

pub(super) fn half_height(layout: &Layout) -> f32 {
    (layout.field[1] * 0.75).min(160.0 * layout.unit)
}

impl Tape<'_> {
    pub(super) fn draw(
        &self,
        layout: &Layout,
        scene: &mut SceneWriter<'_>,
    ) -> Result<(), PanelDrawError> {
        let u = layout.unit;
        let cy = layout.centre[1];
        let half = half_height(layout);
        let status = shown_status(self.signal);
        let color = status_color(layout, status);
        let x0 = self.index;
        halo_polyline(
            layout,
            scene,
            &[[x0, cy - half], [x0, cy + half]],
            color,
            1.4 * u,
        )?;
        scene.fill_color(HUD_GREEN)?;
        let anchor = if self.side < 0.0 {
            Anchor::MIDDLE_RIGHT
        } else {
            Anchor::MIDDLE_LEFT
        };
        scene.text(
            x0,
            cy - half - 10.0 * u,
            layout.text(11.0),
            anchor,
            self.label,
        )?;
        if status.shows_value() {
            self.marks(layout, half, color, scene)?;
        }
        self.pointer(layout, status, scene)?;
        if status != SignalStatus::Valid {
            scene.fill_color(color)?;
            scene.text(
                x0,
                cy + half + 12.0 * u,
                layout.text(10.0),
                anchor,
                status_label(status),
            )?;
        }
        Ok(())
    }

    fn marks(
        &self,
        layout: &Layout,
        half: f32,
        color: Rgba8,
        scene: &mut SceneWriter<'_>,
    ) -> Result<(), PanelDrawError> {
        let u = layout.unit;
        let cy = layout.centre[1];
        let value = self.signal.value;
        let per_unit = half / self.span;
        let reach = 70.0 * u;
        let left = if self.side < 0.0 {
            self.index - reach
        } else {
            self.index
        };
        scene.save()?;
        scene.clip_rect(left, cy - half, reach, half * 2.0)?;
        let first = libm::floorf((value - self.span) / self.minor) as i32;
        let last = libm::ceilf((value + self.span) / self.minor) as i32;
        for step in first..=last {
            let mark = step as f32 * self.minor;
            if self.floor.is_some_and(|floor| mark < floor) {
                continue;
            }
            let y = cy - (mark - value) * per_unit;
            let major = libm::fmodf(libm::fabsf(mark) + 0.5, self.major) < 1.0;
            let length = if major { 10.0 } else { 5.0 } * u;
            halo_polyline(
                layout,
                scene,
                &[[self.index, y], [self.index + self.side * length, y]],
                color,
                1.3 * u,
            )?;
            let clear = libm::fabsf(y - cy);
            if major && clear > 16.0 * u && clear < half - 7.0 * u {
                let text = fmt_label!(12, "{:.0}", mark);
                let anchor = if self.side < 0.0 {
                    Anchor::MIDDLE_RIGHT
                } else {
                    Anchor::MIDDLE_LEFT
                };
                scene.fill_color(color)?;
                scene.text_attributed(
                    self.group.to_u8(),
                    self.index + self.side * 14.0 * u,
                    y,
                    layout.text(11.0),
                    anchor,
                    text.as_str(),
                )?;
            }
        }
        scene.restore()?;
        Ok(())
    }

    /// The boxed exact value at the index, with a notch that touches the tape.
    fn pointer(
        &self,
        layout: &Layout,
        status: SignalStatus,
        scene: &mut SceneWriter<'_>,
    ) -> Result<(), PanelDrawError> {
        let u = layout.unit;
        let cy = layout.centre[1];
        let (s, x0) = (self.side, self.index);
        let near = x0 + s * 8.0 * u;
        let far = x0 + s * 70.0 * u;
        let h = 11.0 * u;
        let outline = [
            [x0 + s * 2.0 * u, cy],
            [near, cy - h * 0.5],
            [near, cy - h],
            [far, cy - h],
            [far, cy + h],
            [near, cy + h],
            [near, cy + h * 0.5],
            [x0 + s * 2.0 * u, cy],
        ];
        let color = status_color(layout, status);
        halo_polyline(layout, scene, &outline, color, 1.5 * u)?;
        scene.fill_color(color)?;
        let mid = (near + far) * 0.5;
        if status.shows_value() {
            let number = fmt_label!(16, "{:.0}", self.signal.value);
            scene.text_attributed(
                self.group.to_u8(),
                mid,
                cy,
                layout.text(14.0),
                Anchor::CENTER,
                number.as_str(),
            )?;
        } else {
            scene.text(mid, cy, layout.text(14.0), Anchor::CENTER, "---")?;
        }
        Ok(())
    }
}

/// Scale position of a vertical speed: linear to 1000 fpm, compressed to 6000 fpm.
pub(super) fn vertical_fraction(fpm: f32) -> f32 {
    let magnitude = libm::fabsf(fpm).min(6000.0);
    let fraction = if magnitude <= 1000.0 {
        magnitude / 2000.0
    } else {
        0.5 + (magnitude - 1000.0) / 10_000.0
    };
    if fpm < 0.0 { -fraction } else { fraction }
}

/// An analog vertical speed scale with a moving caret; its ticks carry no numerals.
pub(super) fn vertical_speed(
    data: &PanelData,
    x: f32,
    layout: &Layout,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let u = layout.unit;
    let cy = layout.centre[1];
    let reach = half_height(layout) * 0.8;
    halo_polyline(
        layout,
        scene,
        &[[x, cy - reach], [x, cy + reach]],
        HUD_GREEN,
        1.2 * u,
    )?;
    for fpm in [
        -6000.0, -2000.0, -1000.0, -500.0, 0.0, 500.0, 1000.0, 2000.0, 6000.0,
    ] {
        let y = cy - vertical_fraction(fpm) * reach;
        let major = fpm == 0.0 || libm::fabsf(fpm) == 1000.0 || libm::fabsf(fpm) == 6000.0;
        halo_polyline(
            layout,
            scene,
            &[[x, y], [x - if major { 7.0 } else { 4.0 } * u, y]],
            HUD_GREEN,
            1.2 * u,
        )?;
    }
    scene.fill_color(HUD_GREEN)?;
    scene.text(
        x,
        cy - reach - 10.0 * u,
        layout.text(10.0),
        Anchor::CENTER,
        "VS",
    )?;
    let status = shown_status(data.vsi_fpm);
    if !status.shows_value() {
        scene.fill_color(status_color(layout, status))?;
        return Ok(scene.text(
            x,
            cy + reach + 10.0 * u,
            layout.text(10.0),
            Anchor::CENTER,
            status_label(status),
        )?);
    }
    let y = cy - vertical_fraction(data.vsi_fpm.value) * reach;
    let tip = x - 2.0 * u;
    let color = status_color(layout, status);
    halo_polyline(
        layout,
        scene,
        &[
            [tip, y],
            [tip - 9.0 * u, y - 5.0 * u],
            [tip - 9.0 * u, y + 5.0 * u],
            [tip, y],
        ],
        color,
        1.6 * u,
    )
}
