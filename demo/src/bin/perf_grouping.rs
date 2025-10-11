// demo/src/bin/perf_grouping.rs
//! Visual perf bench: Places (simple) vs GroupedPlaces (clustering) on ~2,000 points.
//! - Space: toggle Places / GroupedPlaces
//! - Alt   : force continuous repaint (stabilize the average)
//! - "Reset avg": reset the rolling average

use std::time::{Duration, Instant};

use rand::{rngs::StdRng, Rng, SeedableRng};

use walkers::{lon_lat, Map, MapMemory};
use walkers::extras::{
    GroupedPlaces, LabeledSymbol, LabeledSymbolGroup, LabeledSymbolGroupStyle, LabeledSymbolStyle,
    Places, Symbol,
};

/// Small rolling average over last N frames.
struct RollingAvg<const N: usize> {
    buf: [f64; N],
    i: usize,
    n: usize,
}
impl<const N: usize> RollingAvg<N> {
    fn new() -> Self {
        Self { buf: [0.0; N], i: 0, n: 0 }
    }
    fn reset(&mut self) {
        self.i = 0;
        self.n = 0;
        self.buf.fill(0.0);
    }
    fn push_ms(&mut self, v_ms: f64) {
        self.buf[self.i % N] = v_ms;
        self.i += 1;
        self.n = self.n.saturating_add(1).min(N);
    }
    fn mean(&self) -> f64 {
        let n = self.n.max(1);
        self.buf[..n].iter().sum::<f64>() / n as f64
    }
}
// Fix: implement Default so we can store it easily elsewhere if needed.
impl<const N: usize> Default for RollingAvg<N> {
    fn default() -> Self { Self::new() }
}

/// Build N labeled points inside a bbox (Paris area) for repeatable perf tests.
fn make_points(n: usize) -> Vec<LabeledSymbol> {
    let mut rng = StdRng::seed_from_u64(42);
    let (lon0, lon1) = (2.25_f64, 2.45_f64);
    let (lat0, lat1) = (48.80_f64, 48.92_f64);

    (0..n)
        .map(|i| {
            let lon = rng.gen_range(lon0..lon1);
            let lat = rng.gen_range(lat0..lat1);
            let mut style = LabeledSymbolStyle::default();
            style.symbol_size = 5.0; // keep glyph drawing cheap

            LabeledSymbol {
                position: lon_lat(lon, lat),
                label: format!("P{i}"),
                symbol: Some(Symbol::Circle("•".to_string())),
                style,
            }
        })
        .collect()
}

/// Plain `Places` plugin.
fn make_places_plugin(points: Vec<LabeledSymbol>) -> Places<LabeledSymbol> {
    Places::new(points)
}

/// `GroupedPlaces` with the default bubble/cluster style.
fn make_grouped_plugin(
    points: Vec<LabeledSymbol>,
) -> GroupedPlaces<LabeledSymbol, LabeledSymbolGroup> {
    let group_style = LabeledSymbolGroup {
        style: LabeledSymbolGroupStyle::default(),
    };
    GroupedPlaces::new(points, group_style)
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Walkers perf: GroupedPlaces vs Places",
        options,
        Box::new(|_cc| Ok(Box::<PerfApp>::default())),
    )
}

#[derive(Default)]
struct PerfApp {
    memory: MapMemory,
    points: Option<Vec<LabeledSymbol>>,
    use_grouped: bool,
    avg: RollingAvg<120>,
}

impl eframe::App for PerfApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Initialize dataset once.
        if self.points.is_none() {
            self.points = Some(make_points(2_000));
        }
        let points = self.points.as_ref().unwrap().clone();

        // Top bar (controls)
        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.heading("Perf test • 2,000 places");
            ui.label("Space = toggle Places/Grouped • Alt = continuous repaint");
            ui.horizontal(|ui| {
                ui.toggle_value(&mut self.use_grouped, "Use GroupedPlaces");
                if ui.button("Reset avg").clicked() {
                    self.avg.reset();
                }
            });
        });

        // Center panel: map + plugin (two explicit branches to avoid type mismatch)
        egui::CentralPanel::default().show(ctx, |ui| {
            let my_pos = lon_lat(2.3522, 48.8566);
            let map = Map::new(None, &mut self.memory, my_pos);

            let t0 = Instant::now();
            let response = if self.use_grouped {
                let plugin = make_grouped_plugin(points.clone());
                ui.add(map.with_plugin(plugin))
            } else {
                let plugin = make_places_plugin(points.clone());
                ui.add(map.with_plugin(plugin))
            };
            let dt_ms = t0.elapsed().as_secs_f64() * 1_000.0;
            self.avg.push_ms(dt_ms);

            // Overlay the current rolling average
            let label = if self.use_grouped { "GroupedPlaces" } else { "Places" };
            let painter = ui.painter_at(response.rect);
            let text = format!("{label}: avg {:.1} ms / frame", self.avg.mean());
            painter.text(
                response.rect.left_top() + egui::vec2(8.0, 8.0),
                egui::Align2::LEFT_TOP,
                text,
                egui::TextStyle::Body.resolve(ui.style()),
                ui.style().visuals.text_color(),
            );
        });

        // Space toggles mode; Alt enforces repaint (stabilize the average)
        if ctx.input(|i| i.key_pressed(egui::Key::Space)) {
            self.use_grouped = !self.use_grouped;
            self.avg.reset();
        }
        if ctx.input(|i| i.modifiers.alt) {
            ctx.request_repaint();
        } else {
            ctx.request_repaint_after(Duration::from_millis(16));
        }
    }
}
