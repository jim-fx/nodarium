use myzel_core::{grow, GrowthParams, Tissue};
fn main() {
    let h = myzel_core::grow_gills(&grow(&GrowthParams::default()), &Default::default());
    for t in [Tissue::Gill, Tissue::Veil] { println!("{:?} {}", t, h.segments.iter().filter(|s| s.tissue == t).count()); }
    section(&h, "/tmp/claude-1000/-home-max-Projects-nodarium/69dfb3eb-393a-4a16-b996-f9868fca5966/scratchpad/section.ppm");
    let r = |p: glam::Vec3| (p.x * p.x + p.z * p.z).sqrt();
    let mut stipe: Vec<(f32, f32)> = h.segments.iter().filter(|s| s.tissue == Tissue::Stipe).map(|s| (r(s.end), s.end.y)).collect();
    stipe.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    let pct = |v: &Vec<(f32, f32)>, q: f32| v[((v.len() - 1) as f32 * q) as usize];
    println!("total {} stipe {} r50 {:?} r90 {:?} r98 {:?}", h.len(), stipe.len(), pct(&stipe, 0.5), pct(&stipe, 0.9), pct(&stipe, 0.98));
    let ymax = stipe.iter().map(|s| s.1).fold(0.0f32, f32::max);
    println!("stipe ymax {}", ymax);
    let cap: Vec<_> = h.segments.iter().filter(|s| matches!(s.tissue, Tissue::Cap | Tissue::CapCuticle)).map(|s| s.end).collect();
    let (mut ylo, mut yhi, mut rhi) = (1f32, -1f32, 0f32);
    for p in &cap { ylo = ylo.min(p.y); yhi = yhi.max(p.y); rhi = rhi.max(r(*p)); }
    println!("cap {} y {}..{} rmax {}", cap.len(), ylo, yhi, rhi);
    // radial profile: for r bins, min and max y of cap
    for b in 0..12 {
        let (r0, r1) = (rhi * b as f32 / 12.0, rhi * (b + 1) as f32 / 12.0);
        let ys: Vec<f32> = cap.iter().filter(|p| r(**p) >= r0 && r(**p) < r1).map(|p| p.y).collect();
        let st = stipe.iter().filter(|p| p.0 >= r0 && p.0 < r1).count();
        if ys.is_empty() { println!("r {:.4} none stipe {}", r0, st); continue; }
        let lo = ys.iter().cloned().fold(1.0, f32::min); let hi = ys.iter().cloned().fold(-1.0, f32::max);
        println!("r {:.4}-{:.4} n {} y {:.4}..{:.4} stipe {}", r0, r1, ys.len(), lo, hi, st);
    }
}
pub fn section(h: &myzel_core::Hyphae, path: &str) {
    let (w, hgt, scale) = (600usize, 600usize, 600.0 / 0.035);
    let mut img = vec![[255u8; 3]; w * hgt];
    for (a, b, _, t) in h.iter() {
        for k in 0..8 {
            let p = a + (b - a) * (k as f32 / 8.0);
            if p.z.abs() > 0.0008 { continue; }
            let x = (p.x * scale + w as f32 / 2.0) as i32; let y = hgt as i32 - 20 - (p.y * scale) as i32;
            if x < 0 || y < 0 || x >= w as i32 || y >= hgt as i32 { continue; }
            img[y as usize * w + x as usize] = match t { Tissue::Mycelium => [120,120,120], Tissue::Stipe => [200,170,40], Tissue::Cap => [200,100,40], Tissue::Gill => [220,40,120], Tissue::Veil => [60,60,230], Tissue::CapCuticle => [90,50,20] };
        }
    }
    let mut out = format!("P3 {} {} 255\n", w, hgt);
    for px in img { out += &format!("{} {} {}\n", px[0], px[1], px[2]); }
    std::fs::write(path, out).unwrap();
}
