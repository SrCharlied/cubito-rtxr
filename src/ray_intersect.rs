use crate::hit::Hit;
use crate::ray::Ray;

/// Lo que sabe hacer una primitiva trazable: decir si un rayo la toca y, si
/// lo hace, describir el impacto.
///
/// La respuesta es «no toco» o «toco, y esto es lo que hay ahi», que en
/// Rust es exactamente un `Option`: no hace falta una bandera
/// `is_intersecting` ni un impacto vacio con datos de mentira.
pub trait RayIntersect {
    fn ray_intersect(&self, ray: &Ray) -> Option<Hit>;
}
