use crate::ray::Ray;
use nalgebra_glm::Vec3;

/// Caja alineada a los ejes. Es la geometria del cubo y nada mas: no sabe
/// de caras, normales ni texturas, solo de que tramo del rayo cae adentro.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

/// Tramo del rayo que queda dentro de la caja.
///
/// Ademas de las dos distancias guarda **por que eje** entro y salio. El
/// cuboide necesita ese dato para saber que cara toco, y recalcularlo
/// despues significaria repetir el mismo slab test dos veces.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RayInterval {
    pub t_enter: f32,
    pub t_exit: f32,
    pub enter_axis: usize,
    pub exit_axis: usize,
}

impl Aabb {
    pub fn new(min: Vec3, max: Vec3) -> Self {
        Aabb { min, max }
    }

    /// Construye la caja a partir de dos esquinas cualesquiera, ordenando
    /// cada eje. Evita el error de pasarlas al reves y quedarse con una
    /// caja vacia que nunca impacta.
    pub fn from_corners(a: Vec3, b: Vec3) -> Self {
        Aabb {
            min: Vec3::new(a.x.min(b.x), a.y.min(b.y), a.z.min(b.z)),
            max: Vec3::new(a.x.max(b.x), a.y.max(b.y), a.z.max(b.z)),
        }
    }

    pub fn centro(&self) -> Vec3 {
        (self.min + self.max) * 0.5
    }

    pub fn contiene(&self, punto: &Vec3) -> bool {
        (0..3).all(|eje| punto[eje] >= self.min[eje] && punto[eje] <= self.max[eje])
    }

    /// Slab test: interseca el rayo contra los tres pares de planos y se
    /// queda con la interseccion de los tres intervalos.
    ///
    /// Es el corazon del raycaster. Si los tres tramos se solapan, el rayo
    /// atraviesa la caja; si alguno queda fuera de los otros, pasa de lado.
    ///
    /// `t_min` y `t_max` acotan la busqueda. Pasar `EPSILON` como `t_min`
    /// es lo que evita el autoimpacto de un rayo que nace sobre la
    /// superficie.
    pub fn hit(&self, ray: &Ray, t_min: f32, t_max: f32) -> Option<RayInterval> {
        let mut t_enter = t_min;
        let mut t_exit = t_max;
        let mut enter_axis = 0;
        let mut exit_axis = 0;

        for eje in 0..3 {
            let direccion = ray.direction[eje];
            let origen = ray.origin[eje];

            // Rayo paralelo a este par de planos. La division daria
            // infinito y, si el origen cae justo sobre un plano, un NaN que
            // se colaria callado por `max`/`min`. Se resuelve aparte: o el
            // origen ya esta dentro de la franja y el eje no impone
            // restriccion, o esta fuera y no hay impacto posible.
            if direccion.abs() < f32::EPSILON {
                if origen < self.min[eje] || origen > self.max[eje] {
                    return None;
                }
                continue;
            }

            let inversa = 1.0 / direccion;
            let mut t0 = (self.min[eje] - origen) * inversa;
            let mut t1 = (self.max[eje] - origen) * inversa;

            // Con direccion negativa el plano `min` se alcanza despues que
            // el `max`, asi que el par llega invertido.
            if inversa < 0.0 {
                std::mem::swap(&mut t0, &mut t1);
            }

            if t0 > t_enter {
                t_enter = t0;
                enter_axis = eje;
            }
            if t1 < t_exit {
                t_exit = t1;
                exit_axis = eje;
            }

            if t_exit <= t_enter {
                return None;
            }
        }

        Some(RayInterval {
            t_enter,
            t_exit,
            enter_axis,
            exit_axis,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::EPSILON;

    fn cubo_unitario() -> Aabb {
        Aabb::new(Vec3::new(-1.0, -1.0, -1.0), Vec3::new(1.0, 1.0, 1.0))
    }

    #[test]
    fn rayo_frontal_entra_y_sale_por_z() {
        let ray = Ray::new(Vec3::new(0.0, 0.0, 5.0), Vec3::new(0.0, 0.0, -1.0));
        let tramo = cubo_unitario()
            .hit(&ray, EPSILON, f32::INFINITY)
            .expect("debe impactar");

        assert!((tramo.t_enter - 4.0).abs() < 1e-5);
        assert!((tramo.t_exit - 6.0).abs() < 1e-5);
        assert_eq!(tramo.enter_axis, 2);
        assert_eq!(tramo.exit_axis, 2);
    }

    #[test]
    fn rayo_que_pasa_de_lado_falla() {
        let ray = Ray::new(Vec3::new(3.0, 0.0, 5.0), Vec3::new(0.0, 0.0, -1.0));

        assert!(cubo_unitario().hit(&ray, EPSILON, f32::INFINITY).is_none());
    }

    #[test]
    fn rayo_paralelo_dentro_de_la_franja_no_descarta() {
        // Viaja en X puro, con Y y Z dentro de la caja: los dos ejes
        // paralelos no restringen y el impacto lo decide solo X.
        let ray = Ray::new(Vec3::new(-5.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0));
        let tramo = cubo_unitario()
            .hit(&ray, EPSILON, f32::INFINITY)
            .expect("debe impactar");

        assert!((tramo.t_enter - 4.0).abs() < 1e-5);
        assert_eq!(tramo.enter_axis, 0);
    }

    #[test]
    fn rayo_paralelo_fuera_de_la_franja_falla() {
        let ray = Ray::new(Vec3::new(-5.0, 3.0, 0.0), Vec3::new(1.0, 0.0, 0.0));

        assert!(cubo_unitario().hit(&ray, EPSILON, f32::INFINITY).is_none());
    }

    #[test]
    fn from_corners_ordena_las_esquinas() {
        let caja = Aabb::from_corners(Vec3::new(1.0, 1.0, 1.0), Vec3::new(-1.0, -1.0, -1.0));

        assert_eq!(caja, cubo_unitario());
    }

    #[test]
    fn contiene_distingue_dentro_de_fuera() {
        let caja = cubo_unitario();

        assert!(caja.contiene(&Vec3::zeros()));
        assert!(!caja.contiene(&Vec3::new(0.0, 2.0, 0.0)));
    }
}
