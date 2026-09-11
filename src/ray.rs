use nalgebra_glm::Vec3;

/// Un rayo: de donde sale y hacia donde va.
///
/// Empaquetar origen y direccion evita el error clasico de pasarlos
/// invertidos en una firma de dos `Vec3`, y da un lugar natural a `at`.
///
/// La direccion se asume **normalizada**: es lo que hace que el parametro
/// `t` sea una distancia real y no un multiplo arbitrario del vector.
#[derive(Debug, Clone, Copy)]
pub struct Ray {
    pub origin: Vec3,
    pub direction: Vec3,
}

impl Ray {
    pub fn new(origin: Vec3, direction: Vec3) -> Self {
        Ray { origin, direction }
    }

    /// Punto del rayo a distancia `t` del origen.
    pub fn at(&self, t: f32) -> Vec3 {
        self.origin + self.direction * t
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn at_cero_devuelve_el_origen() {
        let ray = Ray::new(Vec3::new(1.0, 2.0, 3.0), Vec3::new(0.0, 0.0, -1.0));

        assert!((ray.at(0.0) - ray.origin).magnitude() < 1e-6);
    }

    #[test]
    fn con_direccion_normalizada_t_es_una_distancia() {
        let direccion = Vec3::new(1.0, 1.0, 0.0).normalize();
        let ray = Ray::new(Vec3::zeros(), direccion);

        let recorrido = (ray.at(5.0) - ray.origin).magnitude();

        assert!((recorrido - 5.0).abs() < 1e-5, "recorrido {recorrido}");
    }
}
