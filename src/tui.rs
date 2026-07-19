use crate::calculator::{self, Inputs};
use crate::region;
use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{
        block::{Position, Title},
        Block, Borders, Paragraph,
    },
    Frame, Terminal,
};
use std::io::{self, stdout};

const EXECUTION_TIMES: &[(&str, f64)] = &[
    ("0.1s", 100.0),
    ("0.2s", 200.0),
    ("0.3s", 300.0),
    ("0.4s", 400.0),
    ("0.5s", 500.0),
    ("1s", 1000.0),
    ("5s", 5000.0),
    ("10s", 10000.0),
    ("20s", 20000.0),
    ("1m", 60000.0),
    ("10m", 600000.0),
    ("30m", 1800000.0),
    ("60m", 3600000.0),
];

struct SliderState {
    cpu: i32,            // 1..=8
    memory_mib: i32,     // 128..=32768
    requests_mil: i32,   // 1..=100
    exec_time_idx: usize, // 0..=12
    region_idx: usize,   // 0..=regions.len()-1
    selected_row: usize, // 0..=4
}

pub fn run() -> Result<()> {
    let regions = region::all();
    let default_region_idx = regions
        .iter()
        .position(|r| r == "us-central1")
        .unwrap_or(0);

    let mut state = SliderState {
        cpu: 1,
        memory_mib: 512,
        requests_mil: 2,
        exec_time_idx: 0,
        region_idx: default_region_idx,
        selected_row: 0,
    };

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Set panic hook to ensure terminal is restored if panic occurs
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        original_hook(panic_info);
    }));

    let result = run_loop(&mut terminal, &mut state, &regions);

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    state: &mut SliderState,
    regions: &[String],
) -> Result<()> {
    loop {
        terminal.draw(|f| draw(f, state, regions))?;

        if let Event::Key(key) = event::read()? {
            if key.kind == event::KeyEventKind::Press {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Up | KeyCode::Char('k') => {
                        state.selected_row = state.selected_row.saturating_sub(1);
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if state.selected_row < 4 {
                            state.selected_row += 1;
                        }
                    }
                    KeyCode::Left | KeyCode::Char('h') => {
                        adjust_slider(state, regions.len(), false, false);
                    }
                    KeyCode::Right | KeyCode::Char('l') => {
                        adjust_slider(state, regions.len(), true, false);
                    }
                    KeyCode::PageUp => {
                        adjust_slider(state, regions.len(), true, true);
                    }
                    KeyCode::PageDown => {
                        adjust_slider(state, regions.len(), false, true);
                    }
                    _ => {}
                }
            }
        }
    }
    Ok(())
}

fn adjust_slider(state: &mut SliderState, region_count: usize, increase: bool, page: bool) {
    match state.selected_row {
        0 => {
            // CPU: 1..=8, step 1
            if increase {
                if state.cpu < 8 {
                    state.cpu += 1;
                }
            } else if state.cpu > 1 {
                state.cpu -= 1;
            }
        }
        1 => {
            // Memory: 128..=32768, step 128, page_step 4096
            let step = if page { 4096 } else { 128 };
            if increase {
                state.memory_mib = (state.memory_mib + step).min(32768);
            } else {
                state.memory_mib = (state.memory_mib - step).max(128);
            }
        }
        2 => {
            // Requests: 1..=100, step 1, page_step 10
            let step = if page { 10 } else { 1 };
            if increase {
                state.requests_mil = (state.requests_mil + step).min(100);
            } else {
                state.requests_mil = (state.requests_mil - step).max(1);
            }
        }
        3 => {
            // Execution time: 0..=12, step 1
            if increase {
                if state.exec_time_idx < EXECUTION_TIMES.len() - 1 {
                    state.exec_time_idx += 1;
                }
            } else if state.exec_time_idx > 0 {
                state.exec_time_idx -= 1;
            }
        }
        4 => {
            // Region: 0..=region_count-1, step 1
            if increase {
                if state.region_idx < region_count - 1 {
                    state.region_idx += 1;
                }
            } else if state.region_idx > 0 {
                state.region_idx -= 1;
            }
        }
        _ => {}
    }
}

fn draw(f: &mut Frame, state: &SliderState, regions: &[String]) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Title
            Constraint::Length(1), // Spacer
            Constraint::Length(3), // CPU slider
            Constraint::Length(3), // Memory slider
            Constraint::Length(3), // Requests slider
            Constraint::Length(3), // Execution duration slider
            Constraint::Length(3), // Region slider
            Constraint::Length(1), // Spacer
            Constraint::Length(5), // Cost box
            Constraint::Min(0),    // Fill spacer
            Constraint::Length(1), // Footer instruction bar
        ])
        .split(f.size());

    // Title
    let title_para = Paragraph::new("Google Cloud Run — Monthly Cost Estimator")
        .alignment(Alignment::Center)
        .style(
            Style::default()
                .fg(Color::Rgb(125, 207, 255)) // TITLE_FG (#7dcfff)
                .add_modifier(Modifier::BOLD),
        );
    f.render_widget(title_para, chunks[0]);

    // Sliders
    render_slider_row(
        f,
        chunks[2],
        "CPU",
        &format!("{} vCPU", state.cpu),
        1,
        8,
        state.cpu,
        state.selected_row == 0,
        Color::Rgb(158, 206, 106), // #9ece6a
    );

    let mem_formatted = if state.memory_mib % 1024 == 0 {
        format!("{} GiB ({} MiB)", state.memory_mib / 1024, state.memory_mib)
    } else {
        format!("{} MiB", state.memory_mib)
    };
    render_slider_row(
        f,
        chunks[3],
        "Memory",
        &mem_formatted,
        128,
        32768,
        state.memory_mib,
        state.selected_row == 1,
        Color::Rgb(224, 175, 104), // #e0af68
    );

    render_slider_row(
        f,
        chunks[4],
        "Requests / month",
        &format!("{} million", state.requests_mil),
        1,
        100,
        state.requests_mil,
        state.selected_row == 2,
        Color::Rgb(125, 207, 255), // #7dcfff
    );

    render_slider_row(
        f,
        chunks[5],
        "Execution time",
        EXECUTION_TIMES[state.exec_time_idx].0,
        0,
        (EXECUTION_TIMES.len() - 1) as i32,
        state.exec_time_idx as i32,
        state.selected_row == 3,
        Color::Rgb(255, 158, 100), // #ff9e64
    );

    let region_name = &regions[state.region_idx];
    let region_tier = region::tier(region_name)
        .map(|t| t.to_string())
        .unwrap_or_default();
    let region_formatted = format!("{} ({})", region_name, region_tier);
    render_slider_row(
        f,
        chunks[6],
        "Region",
        &region_formatted,
        0,
        (regions.len() - 1) as i32,
        state.region_idx as i32,
        state.selected_row == 4,
        Color::Rgb(187, 154, 247), // #bb9af7
    );

    // Cost Box
    let inputs = get_inputs(state, regions);
    let estimate_res = calculator::estimate(&inputs);

    let cost_content = match estimate_res {
        Ok(est) => {
            let total_str = format!("${:.2}", est.total());
            let breakdown_str = format!(
                "CPU ${:.2}    Memory ${:.2}    Requests ${:.2}",
                est.cpu_cost, est.memory_cost, est.request_cost
            );
            let footer_str = "request-based billing · concurrency 1 · free tier ignored";

            let mut lines = Vec::new();
            lines.push(Line::from(vec![
                Span::styled(
                    total_str,
                    Style::default()
                        .fg(Color::Rgb(158, 206, 106)) // COST_FG
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" / month"),
            ]));
            lines.push(Line::from(breakdown_str));
            lines.push(Line::from(Span::styled(
                footer_str,
                Style::default().fg(Color::Rgb(86, 95, 137)), // DIM
            )));
            Text::from(lines)
        }
        Err(e) => Text::from(vec![Line::from(Span::styled(
            format!("Error: {}", e),
            Style::default().fg(Color::Red),
        ))]),
    };

    let cost_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(158, 206, 106))) // COST_FG
        .title(" Estimated monthly cost ")
        .style(Style::default());

    let cost_para = Paragraph::new(cost_content)
        .block(cost_block)
        .style(Style::default());
    f.render_widget(cost_para, chunks[8]);

    // Footer Instruction Bar
    let footer_text = "↑/↓ or j/k: select    ←/→ or h/l: adjust    PgUp/PgDn: jump    q: quit";
    let footer_para = Paragraph::new(footer_text)
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::Rgb(86, 95, 137))); // DIM
    f.render_widget(footer_para, chunks[10]);
}

#[allow(clippy::too_many_arguments)]
fn render_slider_row(
    f: &mut Frame,
    area: Rect,
    name: &str,
    formatted_value: &str,
    min: i32,
    max: i32,
    val: i32,
    is_focused: bool,
    accent_color: Color,
) {
    let border_color = if is_focused {
        accent_color
    } else {
        Color::Rgb(59, 66, 97) // DIMMER (#3b4261)
    };
    let border_style = if is_focused {
        Style::default()
            .fg(border_color)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(border_color)
    };

    let bg_style = if is_focused {
        Style::default().bg(Color::Rgb(40, 52, 87)) // FOCUS_BG (#283457)
    } else {
        Style::default()
    };

    let focus_mark = if is_focused { "▸ " } else { "  " };
    let name_label = format!("{}{}", focus_mark, name);
    let title_left = Span::styled(
        name_label,
        if is_focused {
            Style::default()
                .fg(Color::Rgb(192, 202, 245)) // BRIGHT (#c0caf5)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Rgb(86, 95, 137)) // DIM (#565f89)
        },
    );

    let value_label = format!(" {} ", formatted_value);
    let title_right = Span::styled(
        value_label,
        if is_focused {
            Style::default().fg(accent_color).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Rgb(86, 95, 137)) // DIM (#565f89)
        },
    );

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(
            Title::from(title_left)
                .alignment(Alignment::Left)
                .position(Position::Top),
        )
        .title(
            Title::from(title_right)
                .alignment(Alignment::Right)
                .position(Position::Top),
        )
        .style(bg_style);

    let inner_area = block.inner(area);
    f.render_widget(block, area);

    // Render slider inside block
    if inner_area.width > 0 {
        let width = inner_area.width as i32;
        let pos = if max == min {
            0
        } else {
            (((val - min) as f64 / (max - min) as f64) * (width - 1) as f64).round() as i32
        };

        let mut slider_str = String::with_capacity(width as usize);
        for i in 0..width {
            if i < pos {
                slider_str.push('█');
            } else if i == pos {
                slider_str.push('█');
            } else {
                slider_str.push('░');
            }
        }

        let slider_style = if is_focused {
            Style::default()
                .fg(accent_color)
                .bg(Color::Rgb(40, 52, 87)) // FOCUS_BG
        } else {
            Style::default().fg(Color::Rgb(59, 66, 97)) // DIMMER
        };

        let slider_para = Paragraph::new(slider_str).style(slider_style);
        f.render_widget(slider_para, inner_area);
    }
}

fn get_inputs(state: &SliderState, regions: &[String]) -> Inputs {
    let cpu = state.cpu as f64;
    let memory_mib = state.memory_mib as f64;
    let requests = state.requests_mil as u64 * 1_000_000;
    let avg_request_ms = EXECUTION_TIMES[state.exec_time_idx].1;
    let region = regions[state.region_idx].clone();
    Inputs {
        cpu,
        memory_mib,
        requests,
        region,
        avg_request_ms,
        concurrency: 1,
    }
}
