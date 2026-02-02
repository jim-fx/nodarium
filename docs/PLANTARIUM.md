# Plantarium Documentation

## Encoding Scheme

Plantarium encodes different types of data differently. Right now we have three types of data:

### Path

[0, stem_depth, ... x,y,z,thickness, x,y,z,thickness...]

### Geometry

[1, vertex_amount, face_amount, ... faces in index_a, index_b, index_c ..., ... vertices in x,y,z,x,y,z ..., ... normals in x,y,z,x,y,z ...]

### Instances

[
2,
vertex_amount,
face_amount,\
instance_amount,
stem_depth,
...faces...,
...vertices...,
...normals...,
...4x4 matrices for instances...
]
