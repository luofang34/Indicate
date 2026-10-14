//! Contact symbols, the designation box, off-frame locators, and the range staple.
//!
//! Contacts come from the host, outside the aircraft state ABI, so no contact
//! quantity is drawn as a numeral: position, range, and closure are analog cues.
use super::*;
use crate::context::WeaponMode;
use crate::target::{Contact, Domain, Identity};
use indicate_instrument_symbology::palette;

/// A contact resolved against the camera for one frame.
#[derive(Debug, Clone, Copy)]
pub(super) struct Placed {
    pub body: [f32; 3],
    pub screen: Option<[f32; 2]>,
    pub range_m: f32,
}

pub(super) fn place(contact: &Contact, camera: &Camera, layout: &Layout) -> Option<Placed> {
    if !contact.drawable() {
        return None;
    }
    let body = camera.ned_to_body(contact.relative_position_m)?;
    let margin = 10.0 * layout.unit;
    let screen = camera.project_body(body).map(|p| [p.x, p.y]).filter(|p| {
        p[0] >= margin
            && p[0] <= layout.width - margin
            && p[1] >= margin
            && p[1] <= layout.height - margin
    });
    Some(Placed {
        body,
        screen,
        range_m: contact.range_m()?,
    })
}

fn color(contact: &Contact) -> Rgba8 {
    match (contact.status, contact.identity) {
        (SignalStatus::Valid, Identity::Friendly) => palette::CYAN,
        (SignalStatus::Valid, _) => HUD_GREEN,
        _ => safety::CAUTION_AMBER,
    }
}

pub(super) fn draw(
    context: &HudContext,
    camera: &Camera,
    layout: &Layout,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let picture = &context.targets;
    let designated = picture.designated_index();
    for (index, contact) in picture.contacts().iter().enumerate() {
        let Some(placed) = place(contact, camera, layout) else {
            continue;
        };
        let selected = designated == Some(index);
        match placed.screen {
            Some(at) => symbol(contact, at, selected, layout, scene)?,
            None => edge_cue(contact, &placed, camera, layout, scene)?,
        }
        if selected {
            if placed.screen.is_none() {
                locator(&placed, camera, layout, scene)?;
            }
            range_staple(context, contact, &placed, layout, scene)?;
        }
    }
    Ok(())
}

/// Identity is coded by shape: diamond hostile, circle friendly, square unknown.
fn symbol(
    contact: &Contact,
    at: [f32; 2],
    selected: bool,
    layout: &Layout,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let u = layout.unit;
    let [x, y] = at;
    let r = 7.0 * u;
    let tint = color(contact);
    match contact.identity {
        Identity::Hostile => halo_polyline(
            layout,
            scene,
            &[[x, y - r], [x + r, y], [x, y + r], [x - r, y], [x, y - r]],
            tint,
            1.5 * u,
        )?,
        Identity::Friendly => {
            scene.stroke(layout.halo(), 3.5 * u)?;
            scene.circle(PaintMode::Stroke, x, y, r)?;
            scene.stroke(tint, 1.5 * u)?;
            scene.circle(PaintMode::Stroke, x, y, r)?;
        }
        Identity::Unknown => {
            let s = r * 0.8;
            halo_polyline(
                layout,
                scene,
                &[
                    [x - s, y - s],
                    [x + s, y - s],
                    [x + s, y + s],
                    [x - s, y + s],
                    [x - s, y - s],
                ],
                tint,
                1.5 * u,
            )?
        }
    }
    if contact.domain == Domain::Surface {
        surface_stake([x, y], tint, layout, scene)?;
    }
    if selected {
        let s = 16.0 * u;
        halo_polyline(
            layout,
            scene,
            &[
                [x - s, y - s],
                [x + s, y - s],
                [x + s, y + s],
                [x - s, y + s],
                [x - s, y - s],
            ],
            tint,
            2.0 * u,
        )?;
    }
    if contact.status != SignalStatus::Valid {
        scene.fill_color(tint)?;
        scene.text(
            x,
            y + 26.0 * u,
            layout.text(10.0),
            Anchor::CENTER,
            status_label(contact.status),
        )?;
    }
    Ok(())
}

/// A stem and a ground tick under a surface contact's symbol, so a surface
/// target never reads as an aircraft.
fn surface_stake(
    at: [f32; 2],
    tint: Rgba8,
    layout: &Layout,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let u = layout.unit;
    let [x, y] = at;
    let foot = y + 16.0 * u;
    halo_polyline(layout, scene, &[[x, y + 7.0 * u], [x, foot]], tint, 1.5 * u)?;
    halo_polyline(
        layout,
        scene,
        &[[x - 6.0 * u, foot], [x + 6.0 * u, foot]],
        tint,
        1.5 * u,
    )
}

/// The point where a ray from the boresight along `direction` meets the inset frame edge.
/// The bottom inset clears the simulation and conformality labels.
fn edge_point(layout: &Layout, direction: [f32; 2], inset: f32) -> [f32; 2] {
    let [cx, cy] = layout.centre;
    let reach_x = (layout.width * 0.5 - inset) / direction[0].abs().max(1e-6);
    let vertical = if direction[1] > 0.0 {
        inset.max(48.0 * layout.unit)
    } else {
        inset
    };
    let reach_y = (layout.height * 0.5 - vertical) / direction[1].abs().max(1e-6);
    let t = reach_x.min(reach_y);
    [cx + direction[0] * t, cy + direction[1] * t]
}

/// A chevron on the frame edge that points toward a contact outside the frame.
fn edge_cue(
    contact: &Contact,
    placed: &Placed,
    camera: &Camera,
    layout: &Layout,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let Some(direction) = camera.screen_direction(placed.body) else {
        return Ok(());
    };
    let u = layout.unit;
    let [x, y] = edge_point(layout, direction, 18.0 * u);
    let [dx, dy] = direction;
    let (px, py) = (-dy, dx);
    let back = 10.0 * u;
    let wide = 7.0 * u;
    let points = [
        [x - dx * back + px * wide, y - dy * back + py * wide],
        [x, y],
        [x - dx * back - px * wide, y - dy * back - py * wide],
    ];
    let tint = color(contact);
    halo_polyline(layout, scene, &points, tint, 2.0 * u)?;
    if contact.identity == Identity::Hostile {
        halo_polyline(
            layout,
            scene,
            &[
                [x - dx * back * 1.6, y - dy * back * 1.6],
                [x - dx * back, y - dy * back],
            ],
            tint,
            2.0 * u,
        )?;
    }
    Ok(())
}

/// The target locator line from the boresight toward a designated contact off the frame.
fn locator(
    placed: &Placed,
    camera: &Camera,
    layout: &Layout,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let Some([dx, dy]) = camera.screen_direction(placed.body) else {
        return Ok(());
    };
    let u = layout.unit;
    let [cx, cy] = layout.centre;
    let length = layout.field[0].min(layout.field[1]) * 0.55;
    let start = [cx + dx * 30.0 * u, cy + dy * 30.0 * u];
    let end = [cx + dx * length, cy + dy * length];
    halo_polyline(layout, scene, &[start, end], HUD_GREEN, 2.0 * u)?;
    let (px, py) = (-dy, dx);
    let head = 9.0 * u;
    halo_polyline(
        layout,
        scene,
        &[
            [
                end[0] - dx * head + px * head * 0.6,
                end[1] - dy * head + py * head * 0.6,
            ],
            end,
            [
                end[0] - dx * head - px * head * 0.6,
                end[1] - dy * head - py * head * 0.6,
            ],
        ],
        HUD_GREEN,
        2.0 * u,
    )?;
    if placed.body[0] < 0.0 {
        scene.fill_color(HUD_GREEN)?;
        scene.text(
            end[0] + dx * 16.0 * u,
            end[1] + dy * 16.0 * u,
            layout.text(11.0),
            Anchor::CENTER,
            "AFT",
        )?;
    }
    Ok(())
}

/// Full scale of the range staple for the weapon mode, metres.
fn staple_scale(context: &HudContext) -> f32 {
    let envelope = context.envelope;
    match context.weapon {
        WeaponMode::Navigation => 10_000.0,
        WeaponMode::Gun => envelope.gun_max_range_m * 2.0,
        WeaponMode::ShortRangeMissile => envelope.short_range.max_range_m * 1.25,
        WeaponMode::MediumRangeMissile => envelope.medium_range.max_range_m * 1.25,
        WeaponMode::Rockets => envelope.rocket_max_range_m * 1.5,
        WeaponMode::GuidedBomb => 15_000.0,
        WeaponMode::DirectedEnergy => envelope.energy_max_range_m * 1.5,
    }
    .max(100.0)
}

/// Closure rate: positive when the range decreases, metres per second.
pub(super) fn closure_mps(contact: &Contact, range_m: f32) -> f32 {
    let p = contact.relative_position_m;
    let v = contact.relative_velocity_mps;
    -(p[0] * v[0] + p[1] * v[1] + p[2] * v[2]) / range_m.max(1.0)
}

/// A vertical range scale with envelope marks, a range caret, and a closure arrow.
fn range_staple(
    context: &HudContext,
    contact: &Contact,
    placed: &Placed,
    layout: &Layout,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let u = layout.unit;
    let x = layout.centre[0] + layout.field[0] - 34.0 * u;
    let top = layout.centre[1] + 40.0 * u;
    let height = (layout.field[1] * 0.6).min(120.0 * u);
    if height < 30.0 * u {
        return Ok(());
    }
    let scale = staple_scale(context);
    let y_at = |range: f32| top + height * (1.0 - (range / scale).clamp(0.0, 1.0));
    let tint = color(contact);
    halo_polyline(
        layout,
        scene,
        &[
            [x + 6.0 * u, top],
            [x, top],
            [x, top + height],
            [x + 6.0 * u, top + height],
        ],
        HUD_GREEN,
        1.4 * u,
    )?;
    scene.fill_color(HUD_GREEN)?;
    scene.text(
        x,
        top - 10.0 * u,
        layout.text(10.0),
        Anchor::MIDDLE_LEFT,
        "RNG",
    )?;
    envelope_marks(context, layout, x, &y_at, scene)?;
    let y = y_at(placed.range_m);
    halo_polyline(
        layout,
        scene,
        &[
            [x - 2.0 * u, y],
            [x - 10.0 * u, y - 5.0 * u],
            [x - 10.0 * u, y + 5.0 * u],
            [x - 2.0 * u, y],
        ],
        tint,
        1.6 * u,
    )?;
    closure_arrow(
        closure_mps(contact, placed.range_m),
        [x, y],
        tint,
        layout,
        scene,
    )
}

/// Gun or missile envelope marks on the range scale.
fn envelope_marks(
    context: &HudContext,
    layout: &Layout,
    x: f32,
    y_at: &dyn Fn(f32) -> f32,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let u = layout.unit;
    let envelope = context.envelope;
    let gun = [(envelope.gun_max_range_m, "GUN")];
    let rockets = [(envelope.rocket_max_range_m, "RKT")];
    let energy = [(envelope.energy_max_range_m, "DEW")];
    let missile = envelope.missile(context.weapon).map(|m| {
        [
            (m.max_range_m, "MAX"),
            (m.no_escape_range_m, "NE"),
            (m.min_range_m, "MIN"),
        ]
    });
    let marks: &[(f32, &str)] = match (context.weapon, missile.as_ref()) {
        (WeaponMode::Gun, _) => &gun,
        (WeaponMode::Rockets, _) => &rockets,
        (WeaponMode::DirectedEnergy, _) => &energy,
        (_, Some(missile)) => missile,
        _ => &[],
    };
    for (range, label) in marks {
        let y = y_at(*range);
        halo_polyline(
            layout,
            scene,
            &[[x, y], [x + 12.0 * u, y]],
            HUD_GREEN,
            1.4 * u,
        )?;
        scene.fill_color(HUD_GREEN)?;
        scene.text(
            x + 16.0 * u,
            y,
            layout.text(9.0),
            Anchor::MIDDLE_LEFT,
            label,
        )?;
    }
    Ok(())
}

/// An arrow beside the range caret: down while closing, up while opening.
fn closure_arrow(
    closure: f32,
    [x, y]: [f32; 2],
    tint: Rgba8,
    layout: &Layout,
    scene: &mut SceneWriter<'_>,
) -> Result<(), PanelDrawError> {
    let u = layout.unit;
    if closure.abs() > 2.0 {
        let tip = if closure > 0.0 { 9.0 * u } else { -9.0 * u };
        halo_polyline(
            layout,
            scene,
            &[[x - 16.0 * u, y - tip], [x - 16.0 * u, y + tip]],
            tint,
            1.4 * u,
        )?;
        halo_polyline(
            layout,
            scene,
            &[
                [x - 20.0 * u, y + tip * 0.5],
                [x - 16.0 * u, y + tip],
                [x - 12.0 * u, y + tip * 0.5],
            ],
            tint,
            1.4 * u,
        )?;
    }
    Ok(())
}
