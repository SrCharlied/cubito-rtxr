use crate::camera::{Camera, CameraPreset, DEFAULT_VERTICAL_FOV};
use crate::color::Color;
use crate::cuboid::Cuboid;
use crate::hit::Hit;
use crate::light::Light;
use crate::ray::Ray;
use crate::ray_intersect::RayIntersect;
use nalgebra_glm::Vec3;

/// Lado del cubo de la escena, y unidad de medida de todo lo demas: la
/// distancia de la camara y la de la luz se expresan en multiplos de este
/// numero para que cambiarlo no obligue a recolocar nada a mano.
pub const LADO: f32 = 2.0;

/// Una pieza de la escena: una forma y el color que la tine.
///
/// Todavia es solo el albedo. Cuando haya texturas o un termino especular,
/// este es el tipo que crece, y ni el renderer ni la primitiva se enteran.
#[derive(Debug, Clone, Copy)]
pub struct Object {
    pub shape: Cuboid,
    pub albedo: Color,
}

/// Lo que hay que trazar: objetos, luces y el color del vacio.
pub struct Scene {
    pub objects: Vec<Object>,
    pub lights: Vec<Light>,
    pub background: Color,
}

impl Scene {
    /// Impacto **mas cercano** contra la escena, junto con el objeto que se
    /// toco.
    ///
    /// Recorre todo y se queda con el menor `distance`. Con un solo cubo la
    /// busqueda lineal sobra, pero es la forma correcta desde el principio:
    /// quedarse con el primer impacto encontrado dibujaria el objeto
    /// equivocado en cuanto haya dos.
    pub fn cast(&self, ray: &Ray) -> Option<(Hit, &Object)> {
        let mut cercano: Option<(Hit, &Object)> = None;

        for objeto in &self.objects {
            let Some(hit) = objeto.shape.ray_intersect(ray) else {
                continue;
            };

            let mejora = match &cercano {
                Some((anterior, _)) => hit.distance < anterior.distance,
                None => true,
            };

            if mejora {
                cercano = Some((hit, objeto));
            }
        }

        cercano
    }
}

/// La escena del proyecto: un cubo, una luz y el fondo.
///
/// La luz va muy arriba y adelantada, deliberadamente **desalineada** del
/// ojo: puesta detras de la camara, las tres caras visibles recibirian casi
/// la misma cantidad de luz y el cubo se veria como un hexagono plano. Con
/// la luz corrida, el coseno de Lambert vale algo distinto en cada cara,
/// que es exactamente lo que la difusa tiene que demostrar.
pub fn cubito() -> Scene {
    Scene {
        objects: vec![Object {
            shape: Cuboid::cubo(Vec3::zeros(), LADO),
            // Naranja calido: contrasta con el fondo frio y deja ver bien
            // el degradado entre caras.
            albedo: Color::from_hex(0xE2703A),
        }],
        lights: vec![Light::blanca(Vec3::new(1.0, 4.0, 2.0) * LADO * 0.75, 1.25)],
        background: Color::from_hex(0x10141C),
    }
}

/// Encuadre inicial: ojo separado del cubo, ligeramente en alto y girado,
/// para que se vean tres caras desde el arranque.
pub fn camara_inicial() -> Camera {
    let radio = LADO * 3.0;

    Camera::new(
        Vec3::new(radio * 0.55, radio * 0.45, radio * 0.75),
        Vec3::zeros(),
        Vec3::new(0.0, 1.0, 0.0),
        DEFAULT_VERTICAL_FOV,
    )
    // El minimo deja al ojo justo afuera del cubo —media diagonal es
    // `LADO * 0.87`— y el maximo lo aleja sin que la pieza se vuelva un
    // punto.
    .with_radius_limits(LADO * 1.1, LADO * 8.0)
}

/// El encuadre inicial guardado, para la tecla que vuelve a el.
pub fn preset_inicial() -> CameraPreset {
    camara_inicial().preset()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_rayo_central_toca_el_cubo() {
        let scene = cubito();
        let camera = camara_inicial();
        let ray = camera.ray_from_pixel(400, 300, 800, 600);

        let (hit, _) = scene.cast(&ray).expect("el cubo esta encuadrado");

        assert!(hit.front_face);
        assert!(hit.distance > 0.0);
    }

    #[test]
    fn una_esquina_de_la_imagen_ve_el_fondo() {
        let scene = cubito();
        let camera = camara_inicial();

        assert!(scene.cast(&camera.ray_from_pixel(0, 0, 800, 600)).is_none());
    }

    #[test]
    fn cast_devuelve_el_impacto_mas_cercano() {
        // Dos cubos en fila sobre el eje Z; el rayo viene desde +Z, asi que
        // tiene que quedarse con el de adelante aunque este segundo en la
        // lista.
        let lejano = Object {
            shape: Cuboid::cubo(Vec3::new(0.0, 0.0, -4.0), 1.0),
            albedo: Color::new(1.0, 0.0, 0.0),
        };
        let cercano = Object {
            shape: Cuboid::cubo(Vec3::zeros(), 1.0),
            albedo: Color::new(0.0, 1.0, 0.0),
        };
        let scene = Scene {
            objects: vec![lejano, cercano],
            lights: vec![],
            background: Color::black(),
        };

        let ray = Ray::new(Vec3::new(0.0, 0.0, 5.0), Vec3::new(0.0, 0.0, -1.0));
        let (hit, objeto) = scene.cast(&ray).expect("debe impactar");

        assert_eq!(objeto.albedo, cercano.albedo);
        assert!((hit.distance - 4.5).abs() < 1e-5, "{}", hit.distance);
    }

    #[test]
    fn la_camara_inicial_ve_tres_caras() {
        // Las tres normales que miran hacia el ojo deben aparecer entre los
        // impactos de una rejilla gruesa. Es el chequeo de que el encuadre
        // muestra volumen y no una sola cara de frente.
        let scene = cubito();
        let camera = camara_inicial();
        let mut vistas: Vec<Vec3> = Vec::new();

        for y in (0..600).step_by(20) {
            for x in (0..800).step_by(20) {
                let Some((hit, _)) = scene.cast(&camera.ray_from_pixel(x, y, 800, 600)) else {
                    continue;
                };

                if !vistas.iter().any(|n| (n - hit.normal).magnitude() < 1e-4) {
                    vistas.push(hit.normal);
                }
            }
        }

        assert_eq!(vistas.len(), 3, "caras vistas: {vistas:?}");
    }

    #[test]
    fn la_luz_no_esta_alineada_con_la_camara() {
        // Si lo estuviera, la difusa seria casi plana en las tres caras
        // visibles y no se leeria el volumen.
        let scene = cubito();
        let camera = camara_inicial();

        let hacia_luz = scene.lights[0].position.normalize();
        let hacia_ojo = camera.eye.normalize();
        let alineacion = nalgebra_glm::dot(&hacia_luz, &hacia_ojo);

        assert!(alineacion < 0.95, "alineacion {alineacion}");
    }
}
