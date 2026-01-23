use nodarium_macros::nodarium_definition_file;
use nodarium_macros::nodarium_execute;
use nodarium_utils::{
    encode_float, evaluate_float, geometry::calculate_normals, wrap_arg,
    read_i32_slice
};

nodarium_definition_file!("src/input.json");

#[nodarium_execute]
pub fn execute(size: (i32, i32)) -> Vec<i32> {

    let args = read_i32_slice(size);

    let size = evaluate_float(&args);

    let p = encode_float(size);
    let n = encode_float(-size);

    // [[1,3, x, y, z, x, y,z,x,y,z]];
    let mut cube_geometry = [

        1,          // 1: geometry
        8,          // 8 vertices
        12,         // 12 faces
        
        /*
        Indeces:
        5----6
        | 4--+-7
        0-|--1 |
          3----2

        */
        // this are the indeces for the face
        0, 1, 2, 
        0, 2, 3, 
        0, 3, 4, 
        4, 5, 0, 
        6, 1, 0, 
        5, 6, 0, 
        7, 2, 1, 
        6, 7, 1, 
        2, 7, 3, 
        3, 7, 4, 
        7, 6, 4, 
        4, 6, 5,
        
        // Bottom plate
        p, n, n,
        p, n, p,
        n, n, p,
        n, n, n,

        // Top Plate
        n, p, n,
        p, p, n,
        p, p, p,
        n, p, p,

        // this is the normal for every single vert 1065353216 == 1.0f encoded is i32
        0, 0, 0,
        0, 0, 0,
        0, 0, 0,
        0, 0, 0,
        0, 0, 0,
        0, 0, 0,
        0, 0, 0,
        0, 0, 0,

    ];

    
    calculate_normals(&mut cube_geometry);

    let res = wrap_arg(&cube_geometry);

    res

}
