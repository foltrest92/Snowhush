//! Window decorations, independent of snowfall and its accumulation settings.
use super::*;

#[derive(Default)]
pub(super) struct Garlands {
    enabled: bool,
    windows: Vec<WindowSurface>,
}

impl SnowSimulation {
    pub fn set_garlands(&mut self, enabled: bool) {
        self.restyled |= self.garlands.enabled != enabled;
        self.garlands.enabled = enabled;
    }

    pub fn set_garland_windows(&mut self, windows: &[WindowSurface]) {
        if self.garlands.windows != windows {
            self.garlands.windows = windows.iter().take(32).cloned().collect();
            self.restyled = true;
        }
    }

    fn garland_span(&self, w: &WindowSurface) -> (f32, f32) {
        ((w.frame.x + 8.0).max(0.0), (w.frame.max_x() - 8.0).min(self.width))
    }

    pub(super) fn garland_sprite_count(&self) -> usize {
        if !self.garlands.enabled {
            return 0;
        }
        self.garlands
            .windows
            .iter()
            .map(|w| {
                let (left, right) = self.garland_span(w);
                let width = (right - left).max(0.0);
                (width / 5.0).ceil() as usize + (width / 26.0).ceil() as usize + 4
            })
            .sum()
    }

    pub(super) fn garland_sprites(&self) -> Vec<Sprite> {
        let mut sprites = Vec::new();
        if !self.garlands.enabled {
            return sprites;
        }
        const COLORS: [[f32; 3]; 5] = [[1.0, 0.12, 0.08], [1.0, 0.65, 0.12], [0.12, 0.9, 0.32], [0.18, 0.42, 1.0], [0.95, 0.25, 0.65]];
        for w in &self.garlands.windows {
            let (left, right) = self.garland_span(w);
            let top = w.frame.max_y();
            if right <= left || top > self.height + 5.0 || top < 12.0 {
                continue;
            }
            let visible = |p: V2, margin: f32| {
                !w.cover
                    .iter()
                    .chain(&w.blockers)
                    .any(|r| p.x + margin > r.min_x() && p.x - margin < r.max_x() && p.y + margin > r.min_y() && p.y - margin < r.max_y())
            };
            // A continuous cord, hung from clips every 156 points; its sag remains anchored to the window.
            let cord = |x: f32| {
                let t = ((x - w.frame.x - 8.0) / 156.0).rem_euclid(1.0);
                V2::new(x, top + 2.0 - 9.0 * 4.0 * t * (1.0 - t))
            };
            let count = ((right - left) / 5.0).ceil() as usize;
            for n in 0..count {
                let a = cord(left + n as f32 * 5.0);
                let b = cord((left + (n + 1) as f32 * 5.0).min(right));
                let middle = (a + b) * 0.5;
                if !visible(middle, 3.0) {
                    continue;
                }
                let size = (b - a).length() + 2.0;
                sprites.push(Sprite {
                    position: middle,
                    size,
                    alpha: 1.0,
                    petal: -4.0,
                    drop: [(b.x - a.x) / size * 0.5, (b.y - a.y) / size * 0.5, 0.0, 0.0],
                    ..Sprite::default()
                });
            }
            let first = ((left - w.frame.x - 15.0) / 26.0).ceil() as i32;
            let last = ((right - w.frame.x - 15.0) / 26.0).floor() as i32;
            for n in first..=last {
                let x = w.frame.x + 15.0 + n as f32 * 26.0;
                let at = cord(x) + V2::new(0.0, -5.0);
                if !visible(at, 10.0) {
                    continue;
                }
                let group = n.rem_euclid(4) as f32;
                let phase = self.air.clock as f32 * 1.65 + group * FRAC_PI_2 + (w.id.unsigned_abs() % 97) as f32 * 0.13;
                // Slow staggered fades, with a warm filament that never goes completely black.
                let brightness = 0.16 + 0.84 * smoothstep(-0.45, 0.65, phase.sin());
                sprites.push(Sprite {
                    position: at,
                    size: 24.0,
                    alpha: brightness,
                    petal: -5.0,
                    tint: COLORS[n.rem_euclid(5) as usize],
                    ..Sprite::default()
                });
            }
        }
        sprites
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn window() -> WindowSurface {
        WindowSurface {
            id: 7,
            frame: Rect { x: 100.0, y: 100.0, width: 400.0, height: 250.0 },
            blockers: vec![],
            cover: vec![],
            corner_radius: 8.0,
            corner_exponent: 2.5,
        }
    }
    #[test]
    fn lights_follow_windows_blink_and_switch_off_without_snow() {
        let mut s = SnowSimulation::new(800.0, 600.0, 0.0);
        let mut w = window();
        s.set_garland_windows(&[w.clone()]);
        assert!(s.garland_sprites().is_empty());
        s.set_garlands(true);
        let first = s.garland_sprites();
        assert!(!first.is_empty());
        assert!(first.len() <= s.garland_sprite_count());
        s.air.clock += 1.0;
        assert!(first.iter().zip(s.garland_sprites()).any(|(a, b)| a.petal == -5.0 && (a.alpha - b.alpha).abs() > 0.1));
        w.frame.x += 40.0;
        s.set_garland_windows(&[w]);
        assert!((s.garland_sprites()[0].position.x - first[0].position.x - 40.0).abs() < 0.01);
        s.set_garlands(false);
        assert!(s.garland_sprites().is_empty());
        assert_eq!(s.garland_sprite_count(), 0);
    }
    #[test]
    fn closed_and_covered_windows_have_no_lights() {
        let mut s = SnowSimulation::new(800.0, 600.0, 0.0);
        let mut w = window();
        w.cover.push(Rect { x: 0.0, y: 0.0, width: 800.0, height: 600.0 });
        s.set_garlands(true);
        s.set_garland_windows(&[w]);
        assert!(s.garland_sprites().is_empty());
        s.set_garland_windows(&[]);
        assert!(s.garland_sprites().is_empty());
    }
}
