use std::{
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Context, Result};
use image::{imageops, ImageBuffer, Rgba, RgbaImage};

use crate::ui::WidgetId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AutomationMode {
    Smoke,
}

#[derive(Clone, Debug)]
pub struct AutomationConfig {
    pub mode: AutomationMode,
    pub dump_dir: PathBuf,
}

#[derive(Clone, Debug)]
pub enum AutomationAction {
    SetPreview {
        text: String,
        mouse_buttons: [bool; 5],
    },
    Capture(&'static str),
    CompareToReference {
        capture_name: &'static str,
        reference: PathBuf,
    },
    ClickWidget(WidgetId),
    VerifyTransparency(&'static str),
    Exit,
}

#[derive(Clone, Debug)]
struct AutomationStep {
    after: Duration,
    action: AutomationAction,
}

#[derive(Clone, Debug, Default)]
pub struct AutomationReport {
    lines: Vec<String>,
}

impl AutomationReport {
    pub fn push(&mut self, line: impl Into<String>) {
        self.lines.push(line.into());
    }

    pub fn write_to(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
        }
        fs::write(path, self.to_string()).with_context(|| format!("writing {}", path.display()))?;
        Ok(())
    }
}

impl std::fmt::Display for AutomationReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for line in &self.lines {
            writeln!(f, "{line}")?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct AutomationRunner {
    config: AutomationConfig,
    elapsed: Duration,
    next_step: usize,
    steps: Vec<AutomationStep>,
    report: AutomationReport,
}

impl AutomationRunner {
    pub fn new(config: AutomationConfig) -> Result<Self> {
        fs::create_dir_all(&config.dump_dir)
            .with_context(|| format!("creating {}", config.dump_dir.display()))?;

        let steps = match config.mode {
            AutomationMode::Smoke => vec![
                AutomationStep {
                    after: Duration::from_millis(100),
                    action: AutomationAction::SetPreview {
                        text: "Ctrl + Shift + Alt + A".to_string(),
                        mouse_buttons: [false; 5],
                    },
                },
                AutomationStep {
                    after: Duration::from_millis(250),
                    action: AutomationAction::Capture("normal"),
                },
                AutomationStep {
                    after: Duration::from_millis(320),
                    action: AutomationAction::CompareToReference {
                        capture_name: "normal",
                        reference: PathBuf::from("docs\\screenshot.png"),
                    },
                },
                AutomationStep {
                    after: Duration::from_millis(500),
                    action: AutomationAction::ClickWidget(WidgetId::OverlayButton),
                },
                AutomationStep {
                    after: Duration::from_millis(620),
                    action: AutomationAction::SetPreview {
                        text: "Ctrl + Shift + Alt + A".to_string(),
                        mouse_buttons: [true, false, false, false, false],
                    },
                },
                AutomationStep {
                    after: Duration::from_millis(760),
                    action: AutomationAction::Capture("overlay"),
                },
                AutomationStep {
                    after: Duration::from_millis(820),
                    action: AutomationAction::VerifyTransparency("overlay"),
                },
                AutomationStep {
                    after: Duration::from_millis(980),
                    action: AutomationAction::Exit,
                },
            ],
        };

        Ok(Self {
            config,
            elapsed: Duration::ZERO,
            next_step: 0,
            steps,
            report: AutomationReport::default(),
        })
    }

    pub fn tick(&mut self, delta: Duration) -> Vec<AutomationAction> {
        self.elapsed += delta;
        let mut actions = Vec::new();
        while let Some(step) = self.steps.get(self.next_step) {
            if self.elapsed < step.after {
                break;
            }
            actions.push(step.action.clone());
            self.next_step += 1;
        }
        actions
    }

    pub fn capture_path(&self, name: &str) -> PathBuf {
        self.config.dump_dir.join(format!("{name}.png"))
    }

    pub fn diff_path(&self, name: &str) -> PathBuf {
        self.config.dump_dir.join(format!("{name}-diff.png"))
    }

    pub fn report_path(&self) -> PathBuf {
        self.config.dump_dir.join("automation-report.txt")
    }

    pub fn report_mut(&mut self) -> &mut AutomationReport {
        &mut self.report
    }

    pub fn flush_report(&self) -> Result<()> {
        self.report.write_to(&self.report_path())
    }
}

pub fn save_rgba_png(path: &Path, width: u32, height: u32, bytes: Vec<u8>) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }
    let image: RgbaImage = ImageBuffer::from_raw(width, height, bytes)
        .with_context(|| format!("building RGBA image for {}", path.display()))?;
    image
        .save(path)
        .with_context(|| format!("saving {}", path.display()))?;
    Ok(())
}

pub fn compare_images(captured: &Path, reference: &Path, diff_output: &Path) -> Result<String> {
    let captured_image = image::open(captured)
        .with_context(|| format!("loading {}", captured.display()))?
        .to_rgba8();
    let reference_image = image::open(reference)
        .with_context(|| format!("loading {}", reference.display()))?
        .to_rgba8();

    let resized_capture = if captured_image.dimensions() != reference_image.dimensions() {
        imageops::resize(
            &captured_image,
            reference_image.width(),
            reference_image.height(),
            imageops::FilterType::Lanczos3,
        )
    } else {
        captured_image.clone()
    };

    let mut diff = RgbaImage::new(reference_image.width(), reference_image.height());
    let mut total_error = 0f64;
    let mut alpha_error = 0f64;
    let pixel_count = (reference_image.width() * reference_image.height()) as f64;

    for y in 0..reference_image.height() {
        for x in 0..reference_image.width() {
            let a = resized_capture.get_pixel(x, y);
            let b = reference_image.get_pixel(x, y);
            let dr = u8::abs_diff(a[0], b[0]);
            let dg = u8::abs_diff(a[1], b[1]);
            let db = u8::abs_diff(a[2], b[2]);
            let da = u8::abs_diff(a[3], b[3]);
            diff.put_pixel(x, y, Rgba([dr, dg, db, 255]));
            total_error += (f64::from(dr) + f64::from(dg) + f64::from(db)) / 3.0;
            alpha_error += f64::from(da);
        }
    }

    diff.save(diff_output)
        .with_context(|| format!("saving {}", diff_output.display()))?;

    let rgb_mae = total_error / pixel_count;
    let alpha_mae = alpha_error / pixel_count;
    let mut message = String::new();
    write!(
        &mut message,
        "reference compare: rgb_mae={rgb_mae:.2}, alpha_mae={alpha_mae:.2}, captured={}, reference={}",
        captured.display(),
        reference.display()
    )?;
    Ok(message)
}

pub fn transparency_report(path: &Path) -> Result<String> {
    let image = image::open(path)
        .with_context(|| format!("loading {}", path.display()))?
        .to_rgba8();

    let mut transparent = 0u64;
    let mut opaque = 0u64;
    for pixel in image.pixels() {
        if pixel[3] == 0 {
            transparent += 1;
        } else {
            opaque += 1;
        }
    }

    let total = transparent + opaque;
    let ratio = if total == 0 {
        0.0
    } else {
        transparent as f64 / total as f64
    };
    Ok(format!(
        "overlay transparency: transparent_pixels={transparent}, opaque_pixels={opaque}, transparent_ratio={ratio:.4}"
    ))
}
