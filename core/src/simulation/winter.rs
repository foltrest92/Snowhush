//! Snow sculpting and a plow that visits window ledges before the ground.
use super::*;

#[derive(Default)]
pub(super) struct Winter {
    figures: Vec<Figure>,
    pub rolling: Option<Rolling>,
    pub plow: Option<Plow>,
    powder: Vec<(V2, f32)>,
}

#[cfg(test)]
mod tests {
    use super::*;
    const DT: f32 = 1.0 / 60.0;

    fn snowy() -> SnowSimulation {
        let mut s = SnowSimulation::new(640.0, 480.0, 0.0);
        for i in 0..s.layers[0].count() {
            s.layers[0].set(i, 25.0);
        }
        s.layers[0].refresh_top();
        s
    }

    fn roll(s: &mut SnowSimulation, from: f32, to: f32) {
        s.sculpt_snow(V2::new(from, 25.0), true);
        for n in 1..=20 {
            s.sculpt_snow(V2::new(from + (to - from) * n as f32 / 20.0, 25.0), true);
        }
        s.sculpt_snow(V2::new(to, 25.0), false);
    }

    #[test]
    fn three_mouse_rolled_balls_form_a_snowman_from_real_snow() {
        let mut s = snowy();
        let before: f32 = s.layers[0].heights.iter().sum();
        roll(&mut s, 100.0, 180.0);
        roll(&mut s, 260.0, 190.0);
        roll(&mut s, 350.0, 185.0);
        assert_eq!(s.winter.figures.len(), 1);
        assert_eq!(s.winter.figures[0].radii.len(), 3);
        assert!(s.layers[0].heights.iter().sum::<f32>() < before);
        assert!(s.winter_sprites().iter().any(|s| s.petal == -3.0));
        assert!(s.winter.figures[0].radii.windows(2).all(|r| r[1] < r[0]));
    }

    #[test]
    fn sculpting_requires_snow_and_motion_and_stops_outside_the_screen() {
        let mut s = SnowSimulation::new(640.0, 480.0, 0.0);
        roll(&mut s, 100.0, 180.0);
        assert!(s.winter.figures.is_empty());
        s = snowy();
        for _ in 0..100 {
            s.sculpt_snow(V2::new(100.0, 25.0), true);
        }
        assert_eq!(s.winter.rolling.as_ref().unwrap().area, 0.0);
        s.sculpt_snow(V2::new(-100.0, 25.0), true);
        assert!(s.winter.rolling.is_none());
    }

    #[test]
    fn plow_clears_swept_columns_and_finishes_its_route() {
        let mut s = snowy();
        roll(&mut s, 100.0, 180.0);
        s.start_snowplow();
        s.start_snowplow(); // Repeated menu clicks keep the active pass.
        for _ in 0..1000 {
            s.step_winter(1.0 / 60.0);
        }
        assert!(s.winter.plow.is_none());
        assert!(s.winter.figures.is_empty());
        assert!(s.layers[0].heights.iter().all(|h| *h == 0.0));
    }

    #[test]
    fn clearing_or_leaving_snow_removes_winter_objects() {
        let mut s = snowy();
        roll(&mut s, 100.0, 180.0);
        s.start_snowplow();
        s.blow_away(1.0, false);
        assert!(s.winter_sprites().is_empty());
        s.start_snowplow();
        s.set_modes(&[Mode::Rain]);
        assert!(s.winter_sprites().is_empty());
    }

    #[test]
    fn figures_follow_their_window_and_return_snow_at_its_last_position() {
        let mut s = snowy();
        roll(&mut s, 100.0, 180.0);
        let mut ledge = SnowLayer::new(V2::new(200.0, 200.0), 160, COLUMN_WIDTH, LEDGE_DEPTH, true, (0.0, 0.0));
        for i in 0..ledge.count() {
            ledge.set(i, 20.0);
        }
        ledge.refresh_top();
        s.layers.push(ledge);
        s.ids.push(7);
        s.winter.figures[0].support = 7;
        s.step_winter(DT);
        let x = s.winter_sprites()[0].position.x;
        s.layers[1].origin.x += 50.0;
        s.step_winter(DT);
        assert!((s.winter_sprites()[0].position.x - x - 50.0).abs() < 0.01);
        let before: f32 = s.layers[0].heights.iter().sum::<f32>() * COLUMN_WIDTH;
        let area = s.winter.figures[0].area;
        s.layers.pop();
        s.ids.pop();
        s.step_winter(DT);
        assert!(s.winter.figures.is_empty());
        let after: f32 = s.layers[0].heights.iter().sum::<f32>() * COLUMN_WIDTH;
        assert!((after - before - area).abs() < 1.0);
    }

    #[test]
    fn plow_visits_windows_before_ground_and_skips_closed_windows() {
        let mut s = snowy();
        for (id, y) in [(7, 200.0), (8, 350.0)] {
            let mut layer = SnowLayer::new(V2::new(20.0, y), 40, COLUMN_WIDTH, LEDGE_DEPTH, true, (0.0, 0.0));
            layer.set(10, 12.0);
            layer.refresh_top();
            s.layers.push(layer);
            s.ids.push(id);
        }
        s.start_snowplow();
        assert_eq!(s.winter.plow.as_ref().unwrap().route, vec![0, 7, 8]);
        s.layers.pop();
        s.ids.pop();
        for _ in 0..1000 {
            s.step_winter(DT);
        }
        assert!(s.winter.plow.is_none());
        assert!(s.layers.iter().all(|layer| layer.heights.iter().all(|h| *h == 0.0)));
    }
}

struct Figure {
    support: WindowId,
    x: f32,
    world_x: f32,
    area: f32,
    radii: Vec<f32>,
}

pub(super) struct Rolling {
    support: WindowId,
    x: f32,
    world_x: f32,
    area: f32,
}

pub(super) struct Plow {
    route: Vec<WindowId>,
    x: f32,
    load: f32,
}

impl SnowSimulation {
    /// Hold Ctrl+Alt and move through a drift; releasing the keys places the ball.
    /// A ball released near an unfinished figure becomes its next, smaller tier.
    pub fn sculpt_snow(&mut self, cursor: V2, held: bool) {
        let enabled = self.air.modes.contains(&Mode::Snow) && self.air.gust.is_none() && self.winter.plow.is_none();
        let held = held && enabled && cursor.x >= 0.0 && cursor.x < self.width && cursor.y >= 0.0 && cursor.y < self.height;
        if !held {
            if let Some(ball) = self.winter.rolling.take() {
                self.place_ball(ball);
            }
            return;
        }
        if self.winter.rolling.is_none() && self.winter.figures.len() < 24 {
            let support = self
                .layers
                .iter()
                .enumerate()
                .rev()
                .find(|(li, layer)| {
                    if *li == 0 && !self.ground_settles {
                        return false;
                    }
                    layer
                        .index(cursor.x)
                        .is_some_and(|i| layer.visible[i] && layer.heights[i] - layer.leaves[i] > 1.0 && (cursor.y - layer.surface(i)).abs() < 28.0)
                })
                .map(|(li, _)| li);
            if let Some(li) = support {
                self.winter.rolling = Some(Rolling { support: self.ids[li], x: cursor.x - self.layers[li].base().x, world_x: cursor.x, area: 0.0 });
            }
        }
        let Some(ball) = &mut self.winter.rolling else { return };
        let Some(li) = self.ids.iter().position(|id| *id == ball.support) else { return };
        let layer = &mut self.layers[li];
        let local = (cursor.x - layer.base().x).clamp(0.0, layer.width() - COLUMN_WIDTH);
        // Sample the swept path, bounded so teleporting the cursor cannot harvest a whole window.
        let previous = ball.x;
        ball.x = local;
        ball.world_x = layer.base().x + local;
        let distance = (local - previous).abs().min(60.0);
        if distance < 0.5 {
            return;
        }
        let steps = (distance / COLUMN_WIDTH).ceil() as usize;
        for n in 0..=steps {
            let x = local + (previous - local).signum() * distance * n as f32 / steps.max(1) as f32;
            let i = layer.clamped_index(layer.base().x + x);
            if !layer.visible[i] {
                continue;
            }
            let available = (layer.heights[i] - layer.leaves[i]).max(0.0);
            let take = available.min(((PI * 28.0 * 28.0 - ball.area) / COLUMN_WIDTH).max(0.0));
            layer.remove_snow(take, i);
            ball.area += take * COLUMN_WIDTH;
        }
        layer.refresh_top();
        self.air.heights_changed = true;
    }

    fn place_ball(&mut self, ball: Rolling) {
        if ball.area < PI * 3.0 * 3.0 {
            if let Some(li) = self.ids.iter().position(|id| *id == ball.support) {
                let x = self.layers[li].base().x + ball.x;
                self.layers[li].deposit(ball.area, x, true, Blend::SNOW);
            }
            return;
        }
        let radius = (ball.area / PI).sqrt();
        if let Some(figure) = self
            .winter
            .figures
            .iter_mut()
            .filter(|f| f.support == ball.support && f.radii.len() < 3)
            .filter(|f| (f.x - ball.x).abs() < f.radii[0] + 20.0)
            .min_by(|a, b| (a.x - ball.x).abs().total_cmp(&(b.x - ball.x).abs()))
        {
            // Compact the collected snow into progressively smaller balls.
            figure.radii.push(radius.min(figure.radii.last().unwrap() * 0.78));
            figure.area += ball.area;
        } else {
            self.winter.figures.push(Figure { support: ball.support, x: ball.x, world_x: ball.world_x, area: ball.area, radii: vec![radius] });
        }
        self.restyled = true;
    }

    /// Start one pass, visiting the highest window first and the screen bottom last.
    pub fn start_snowplow(&mut self) {
        if !self.air.modes.contains(&Mode::Snow) || self.winter.plow.is_some() || self.air.gust.is_some() {
            return;
        }
        if let Some(ball) = self.winter.rolling.take() {
            self.place_ball(ball);
        }
        let mut route: Vec<usize> =
            (1..self.layers.len()).filter(|&i| self.layers[i].top > 0.5 || self.winter.figures.iter().any(|f| f.support == self.ids[i])).collect();
        route.sort_by(|&a, &b| self.layers[a].base().y.total_cmp(&self.layers[b].base().y));
        let mut route: Vec<_> = route.into_iter().map(|i| self.ids[i]).collect();
        route.insert(0, 0);
        self.winter.plow = Some(Plow { route, x: -55.0, load: 0.0 });
        self.restyled = true;
    }

    pub(super) fn step_winter(&mut self, dt: f32) {
        // Closed/minimized windows release their figures back into loose snow.
        let mut returned = Vec::new();
        self.winter.figures.retain_mut(|f| {
            let keep = self.ids.contains(&f.support);
            if let Some(li) = self.ids.iter().position(|&id| id == f.support) {
                f.world_x = self.layers[li].base().x + f.x;
            } else {
                returned.push((f.world_x.clamp(0.0, self.width), f.area));
            }
            keep
        });
        if self.winter.rolling.as_ref().is_some_and(|b| !self.ids.contains(&b.support)) {
            let ball = self.winter.rolling.take().unwrap();
            returned.push((ball.world_x.clamp(0.0, self.width), ball.area));
        }
        for (x, area) in returned {
            self.layers[0].deposit(area, x, true, Blend::SNOW);
        }
        self.winter.powder.iter_mut().for_each(|(p, life)| {
            p.x += dt * 70.0;
            p.y += dt * 35.0;
            *life -= dt;
        });
        self.winter.powder.retain(|(_, life)| *life > 0.0);
        let Some(plow) = &mut self.winter.plow else { return };
        let Some(&id) = plow.route.last() else {
            self.winter.plow = None;
            return;
        };
        let Some(li) = self.ids.iter().position(|&i| i == id) else {
            plow.route.pop();
            plow.x = -55.0;
            plow.load = 0.0;
            return;
        };
        let layer = &mut self.layers[li];
        let old = plow.x;
        plow.x += dt * 105.0;
        let blade = plow.x + 37.0;
        let mut removed = false;
        for i in 0..layer.count() {
            let x = (i as f32 + 0.5) * COLUMN_WIDTH;
            if x >= old + 32.0 && x <= blade && layer.heights[i] > 0.0 {
                plow.load += layer.heights[i] * COLUMN_WIDTH;
                layer.set(i, 0.0);
                removed = true;
            }
        }
        self.winter.figures.retain(|f| f.support != id || f.x > blade || f.x < old - 50.0);
        if removed && self.winter.powder.len() < 80 {
            self.winter.powder.push((layer.base() + V2::new(blade, 15.0), 0.65));
        }
        layer.refresh_top();
        self.air.heights_changed = true;
        if plow.x > layer.width() + 55.0 {
            plow.route.pop();
            plow.x = -55.0;
            plow.load = 0.0;
        }
    }

    pub(super) fn winter_sprites(&self) -> Vec<Sprite> {
        let mut sprites = Vec::new();
        for f in &self.winter.figures {
            let Some(li) = self.ids.iter().position(|&id| id == f.support) else { continue };
            let layer = &self.layers[li];
            let x = layer.base().x + f.x;
            let Some(i) = layer.index(x) else { continue };
            if !layer.visible[i] {
                continue;
            }
            let mut y = layer.surface(i) - f.radii[0] * 0.15;
            for (n, &r) in f.radii.iter().enumerate() {
                y += r;
                let head = n == 2;
                let size = r * if head { 3.6 } else { 2.3 };
                sprites.push(Sprite {
                    position: V2::new(x, y),
                    size,
                    alpha: 1.0,
                    petal: if head { -3.0 } else { -1.0 },
                    drop: [r / size, if n == 1 { 1.0 } else { 0.0 }, 0.0, 0.0],
                    ..Sprite::default()
                });
                y += r * 0.70;
            }
        }
        if let Some(ball) = &self.winter.rolling {
            if let Some(li) = self.ids.iter().position(|&id| id == ball.support) {
                let layer = &self.layers[li];
                let x = layer.base().x + ball.x;
                let i = layer.clamped_index(x);
                let r = (ball.area / PI).sqrt().max(1.0);
                sprites.push(Sprite {
                    position: V2::new(x, layer.surface(i) + r),
                    size: r * 2.3,
                    alpha: 1.0,
                    petal: -1.0,
                    drop: [1.0 / 2.3, 0.0, 0.0, 0.0],
                    ..Sprite::default()
                });
            }
        }
        if let Some(plow) = &self.winter.plow {
            if let Some(li) = plow.route.last().and_then(|id| self.ids.iter().position(|i| i == id)) {
                let layer = &self.layers[li];
                let x = layer.base().x + plow.x;
                let i = layer.clamped_index(x);
                if layer.visible[i] {
                    if plow.load > 0.0 {
                        let r = (plow.load / PI).sqrt().min(21.0);
                        sprites.push(Sprite {
                            position: V2::new(x + 42.0, layer.floor(i) + r * 0.55),
                            size: r * 2.3,
                            alpha: 1.0,
                            petal: -1.0,
                            drop: [1.0 / 2.3, 0.0, 0.0, 0.0],
                            ..Sprite::default()
                        });
                    }
                    sprites.push(Sprite { position: V2::new(x, layer.floor(i) + 23.0), size: 105.0, alpha: 1.0, petal: -2.0, ..Sprite::default() });
                }
            }
        }
        for &(p, life) in &self.winter.powder {
            sprites.push(Sprite { position: p, size: 12.0 * life, alpha: life, soft: 1.0, ..Sprite::default() });
        }
        sprites
    }

    pub(super) fn winter_sprite_count(&self) -> usize {
        self.winter.figures.iter().map(|f| f.radii.len()).sum::<usize>()
            + usize::from(self.winter.rolling.is_some())
            + 2 * usize::from(self.winter.plow.is_some())
            + self.winter.powder.len()
    }
}
