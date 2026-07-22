// 3D Perlin noise for organic patterns and paper fiber texture
const PERM: [u8; 256] = [
    151, 160, 137, 91, 90, 15, 131, 13, 201, 95, 96, 53, 194, 233, 7, 225, 140, 36, 103, 30, 69,
    142, 8, 99, 37, 240, 21, 10, 23, 190, 6, 148, 247, 120, 234, 75, 0, 26, 197, 62, 94, 252, 219,
    203, 117, 35, 11, 32, 57, 177, 33, 88, 237, 149, 56, 87, 174, 20, 97, 85, 144, 171, 62, 113,
    138, 102, 158, 99, 186, 212, 127, 80, 116, 123, 6, 147, 93, 191, 140, 113, 128, 116, 111, 115,
    158, 125, 191, 126, 96, 130, 144, 141, 135, 151, 46, 30, 136, 161, 79, 141, 142, 137, 123, 113,
    104, 95, 120, 169, 45, 127, 165, 77, 181, 107, 90, 170, 215, 125, 93, 83, 144, 231, 166, 81,
    54, 147, 190, 119, 168, 220, 162, 144, 138, 154, 145, 157, 155, 118, 181, 127, 98, 93, 168,
    156, 169, 162, 156, 156, 157, 153, 166, 187, 171, 177, 168, 154, 173, 186, 158, 165, 162, 166,
    146, 170, 161, 166, 175, 191, 188, 194, 171, 169, 173, 188, 189, 191, 186, 189, 178, 194, 192,
    195, 189, 187, 190, 199, 195, 199, 195, 200, 194, 195, 194, 195, 200, 197, 199, 198, 198, 203,
    197, 200, 202, 207, 203, 200, 206, 207, 207, 209, 207, 211, 204, 207, 210, 209, 211, 209, 210,
    213, 211, 210, 211, 211, 214, 212, 214, 213, 214, 216, 214, 215, 216, 218, 217, 218, 220, 219,
    221, 220, 222, 224, 222, 223, 225, 225, 227, 226, 227, 228, 229, 230, 232, 231, 233, 232,
];

fn fade(t: f32) -> f32 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + t * (b - a)
}

fn perm_idx(x: i32) -> u8 {
    PERM[(x & 255) as usize]
}

fn grad3(hash: u8, x: f32, y: f32, z: f32) -> f32 {
    let h = hash & 15;
    let u = if h < 8 { x } else { y };
    let v = if h < 4 {
        y
    } else if h == 12 || h == 14 {
        x
    } else {
        z
    };
    (if (h & 1) != 0 { -u } else { u }) + (if (h & 2) != 0 { -v } else { v })
}

/// 3D Perlin noise
pub fn noise3d(x: f32, y: f32, z: f32) -> f32 {
    let xi = x.floor() as i32;
    let yi = y.floor() as i32;
    let zi = z.floor() as i32;
    let xf = x - xi as f32;
    let yf = y - yi as f32;
    let zf = z - zi as f32;

    let u = fade(xf);
    let v = fade(yf);
    let w = fade(zf);

    let p_xi = perm_idx(xi) as i32;
    let p_xi1 = perm_idx(xi + 1) as i32;

    let p0 = perm_idx(p_xi + yi) as i32;
    let p1 = perm_idx(p_xi + yi + 1) as i32;
    let p2 = perm_idx(p_xi1 + yi) as i32;
    let p3 = perm_idx(p_xi1 + yi + 1) as i32;

    let a = perm_idx(p0 + zi);
    let b = perm_idx(p1 + zi);
    let c = perm_idx(p2 + zi);
    let d = perm_idx(p3 + zi);
    let e = perm_idx(p0 + zi + 1);
    let f = perm_idx(p1 + zi + 1);
    let g = perm_idx(p2 + zi + 1);
    let h = perm_idx(p3 + zi + 1);

    let x1 = lerp(grad3(a, xf, yf, zf), grad3(c, xf - 1.0, yf, zf), u);
    let x2 = lerp(
        grad3(b, xf, yf - 1.0, zf),
        grad3(d, xf - 1.0, yf - 1.0, zf),
        u,
    );
    let x3 = lerp(
        grad3(e, xf, yf, zf - 1.0),
        grad3(g, xf - 1.0, yf, zf - 1.0),
        u,
    );
    let x4 = lerp(
        grad3(f, xf, yf - 1.0, zf - 1.0),
        grad3(h, xf - 1.0, yf - 1.0, zf - 1.0),
        u,
    );

    let y1 = lerp(x1, x2, v);
    let y2 = lerp(x3, x4, v);

    lerp(y1, y2, w)
}
