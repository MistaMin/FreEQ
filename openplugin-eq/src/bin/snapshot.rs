// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marcos Quirino Deida
//! Dump the GoodLookin scene so it can be rasterized.

use openplugin::dri::DrawCmd;
use openplugin::dri::goodlookindri::color::Color;
use openplugin::dri::goodlookindri::scene::{Cmd, Point};
use openplugin::dri::goodlookindri::spectrum::SampleRing;
use openplugin::prelude::Plugin;
use openplugin_eq::{render, spectrum_view, EqPlugin};

fn rgba(color: Color) -> String {
    format!("{} {} {} {}", color.r(), color.g(), color.b(), color.a())
}

fn pt(point: Point) -> String {
    format!("{:.2} {:.2}", point.x, point.y)
}

fn main() {
    let width = 980.0f32;
    let height = 460.0f32;
    let mut plugin = EqPlugin::new();
    plugin.prepare(48000.0, 512, 2);
    let ring = SampleRing::new();
    let mut io = vec![vec![0.0_f32; 512]; 2];
    for block in 0..16 {
        for i in 0..512 {
            let n = (block * 512 + i) as f32;
            io[0][i] = (n * 0.13).sin() * 0.2 + (n * 0.05).sin() * 0.12 + (n * 0.31).sin() * 0.05;
            io[1][i] = io[0][i];
        }
        plugin.process(&mut io, 512);
        ring.push(&[&io[0], &io[1]]);
    }
    let mut view = spectrum_view();
    view.update(&ring, 1.0 / 30.0, 48000.0);
    let list = render(&plugin, width, height, Some(&mut view));
    println!("SIZE {width} {height}");
    for cmd in &list.scene.cmds {
        match cmd {
            Cmd::FillEllipse { rect, top, bottom, diagonal } => {
                println!(
                    "FILL_ELLIPSE {:.2} {:.2} {:.2} {:.2} {} {} {}",
                    rect.x, rect.y, rect.w, rect.h, rgba(*top), rgba(*bottom), *diagonal as u8
                );
            }
            Cmd::StrokeEllipse { rect, color, width } => {
                println!(
                    "STROKE_ELLIPSE {:.2} {:.2} {:.2} {:.2} {} {:.2}",
                    rect.x, rect.y, rect.w, rect.h, rgba(*color), width
                );
            }
            Cmd::FillRoundRect { rect, radius, top, bottom, vertical } => {
                println!(
                    "FILL_ROUND {:.2} {:.2} {:.2} {:.2} {:.2} {} {} {}",
                    rect.x, rect.y, rect.w, rect.h, radius, rgba(*top), rgba(*bottom), *vertical as u8
                );
            }
            Cmd::StrokeRoundRect { rect, radius, color, width } => {
                println!(
                    "STROKE_ROUND {:.2} {:.2} {:.2} {:.2} {:.2} {} {:.2}",
                    rect.x, rect.y, rect.w, rect.h, radius, rgba(*color), width
                );
            }
            Cmd::FillRect { rect, top, bottom, vertical } => {
                println!(
                    "FILL_RECT {:.2} {:.2} {:.2} {:.2} {} {} {}",
                    rect.x, rect.y, rect.w, rect.h, rgba(*top), rgba(*bottom), *vertical as u8
                );
            }
            Cmd::Line { a, b, color, width } => {
                println!("LINE {} {} {} {:.2}", pt(*a), pt(*b), rgba(*color), width);
            }
            Cmd::FillPoly { pts, top, bottom, diagonal } => {
                let coords: Vec<String> = pts.iter().map(|p| pt(*p)).collect();
                println!("FILL_POLY {} {} {} {}", coords.len(), coords.join(" "), rgba(*top), rgba(*bottom));
                let _ = diagonal;
            }
            Cmd::StrokePoly { pts, color, width, closed } => {
                let coords: Vec<String> = pts.iter().map(|p| pt(*p)).collect();
                println!(
                    "STROKE_POLY {} {} {} {:.2} {}",
                    coords.len(),
                    coords.join(" "),
                    rgba(*color),
                    width,
                    *closed as u8
                );
            }
            Cmd::StrokeArc { center, radius, from, to, color, width } => {
                println!(
                    "ARC {} {:.2} {:.4} {:.4} {} {:.2}",
                    pt(*center), radius, from, to, rgba(*color), width
                );
            }
            Cmd::Text { at, text, color, size, center } => {
                println!("TEXT {} {:.2} {} {} {}", pt(*at), size, rgba(*color), *center as u8, text.replace('\n', " "));
            }
            Cmd::ClipRound { rect, radius } => {
                println!("CLIP {:.2} {:.2} {:.2} {:.2} {:.2}", rect.x, rect.y, rect.w, rect.h, radius);
            }
            Cmd::PopClip => println!("POP"),
        }
    }
    let byte = |channel: f32| (channel.clamp(0.0, 1.0) * 255.0).round() as u8;
    let rgba = |color: &openplugin::dri::Color| format!("{} {} {} 255", byte(color.r), byte(color.g), byte(color.b));
    for cmd in &list.commands {
        match cmd {
            DrawCmd::FillRect { x, y, w, h, color } => {
                let c = rgba(color);
                println!("FILL_RECT {x:.2} {y:.2} {w:.2} {h:.2} {c} {c} 1");
            }
            DrawCmd::FillRoundedRect { x, y, w, h, radius, color } => {
                let c = rgba(color);
                println!("FILL_ROUND {x:.2} {y:.2} {w:.2} {h:.2} {radius:.2} {c} {c} 1");
            }
            DrawCmd::FillCircle { cx, cy, radius, color } => {
                let c = rgba(color);
                println!(
                    "FILL_ELLIPSE {:.2} {:.2} {:.2} {:.2} {c} {c} 0",
                    cx - radius,
                    cy - radius,
                    radius * 2.0,
                    radius * 2.0
                );
            }
            DrawCmd::Line { x1, y1, x2, y2, color, width } => {
                println!("LINE {x1:.2} {y1:.2} {x2:.2} {y2:.2} {} {width:.2}", rgba(color));
            }
            DrawCmd::Text { text, x, y, size, color, align_center } => {
                println!("TEXT {x:.2} {y:.2} {size:.2} {} {} {text}", rgba(color), *align_center as u8);
            }
            _ => {}
        }
    }
}
