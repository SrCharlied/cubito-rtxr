use crate::ray::Ray;
use nalgebra_glm::{dot, Vec2, Vec3};

/// Todo lo que se sabe de un impacto.
///
/// No carga el material: con un solo cubo seria una copia inutil en el
/// camino caliente, y el renderer ya sabe a que objeto pertenece lo que
/// toco. Lo que si carga es la normal ya orientada y el `uv`, que son los
/// dos datos que el sombreado necesita y la primitiva es la unica que
/// puede calcular.
#[derive(Debug, Clone, Copy)]
pub struct Hit {
    pub distance: f32,
    pub point: Vec3,
    /// Siempre orientada **contra** el rayo. Ver `front_face`.
    pub normal: Vec3,
    pub uv: Vec2,
    /// `true` si el rayo golpeo la cara exterior de la superficie.
    pub front_face: bool,
}

impl Hit {
    /// Construye un impacto orientando la normal contra el rayo.
    ///
    /// `outward_normal` es la normal geometrica, la que apunta hacia afuera
    /// del solido. Un rayo que toca la cara por dentro la recibe apuntando
    /// en el sentido equivocado para iluminar, asi que aqui se voltea y se
    /// recuerda que se volteo.
    pub fn new(ray: &Ray, distance: f32, outward_normal: Vec3, uv: Vec2) -> Self {
        let front_face = dot(&ray.direction, &outward_normal) < 0.0;

        Hit {
            distance,
            point: ray.at(distance),
            normal: if front_face {
                outward_normal
            } else {
                -outward_normal
            },
            uv,
            front_face,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_frontal_apunta_contra_el_rayo() {
        let ray = Ray::new(Vec3::new(0.0, 0.0, 5.0), Vec3::new(0.0, 0.0, -1.0));
        let hit = Hit::new(&ray, 4.0, Vec3::new(0.0, 0.0, 1.0), Vec2::zeros());

        assert!(hit.front_face);
        assert_eq!(hit.normal, Vec3::new(0.0, 0.0, 1.0));
    }

    #[test]
    fn normal_interna_se_invierte() {
        let ray = Ray::new(Vec3::new(0.0, 0.0, 5.0), Vec3::new(0.0, 0.0, -1.0));
        let hit = Hit::new(&ray, 4.0, Vec3::new(0.0, 0.0, -1.0), Vec2::zeros());

        assert!(!hit.front_face);
        assert_eq!(hit.normal, Vec3::new(0.0, 0.0, 1.0));
        assert!(dot(&hit.normal, &ray.direction) < 0.0);
    }

    #[test]
    fn el_punto_se_deriva_del_rayo_y_la_distancia() {
        let ray = Ray::new(Vec3::new(0.0, 0.0, 5.0), Vec3::new(0.0, 0.0, -1.0));
        let hit = Hit::new(&ray, 4.0, Vec3::new(0.0, 0.0, 1.0), Vec2::zeros());

        assert_eq!(hit.point, Vec3::new(0.0, 0.0, 1.0));
    }
}
