use crate::bloom::Bloom;
use crate::camera::{Camera, CameraPreset, DEFAULT_VERTICAL_FOV};
use crate::color::Color;
use crate::cuboid::Cuboid;
use crate::hit::Hit;
use crate::light::Light;
use crate::material::Material;
use crate::ray::Ray;
use crate::ray_intersect::RayIntersect;
use nalgebra_glm::Vec3;

/// Lado del cubo de la escena, y unidad de medida de todo lo demas: la
/// distancia de la camara, la de la luz y la altura del piso se expresan en
/// multiplos de este numero para que cambiarlo no obligue a recolocar nada
/// a mano.
pub const LADO: f32 = 2.0;

/// Altura de la **cara superior** del piso.
///
/// El teseracto no se apoya: flota un poco encima. Apoyado, la sombra nace
/// pegada a la silueta y el charco de luz queda estrangulado justo donde
/// mas se ve; separado, la sombra se despega y se lee como sombra.
pub const ALTURA_DEL_PISO: f32 = -LADO * 1.15;

/// Grosor de la losa. Solo se ve si la camara baja mucho, pero una losa sin
/// grosor seria una cara degenerada y el `uv` de sus costados no
/// significaria nada.
const GROSOR_DEL_PISO: f32 = LADO * 0.3;

/// Lado del piso. Muy grande a proposito: asi su borde queda fuera del
/// alcance de las luces y la losa se desvanece en negro en vez de terminar
/// en una linea recta que se lee como el filo de una mesa. No cuesta nada:
/// sigue siendo un solo AABB.
const EXTENSION_DEL_PISO: f32 = LADO * 60.0;

/// Una pieza de la escena: una forma y como responde a la luz.
#[derive(Debug, Clone, Copy)]
pub struct Object {
    pub shape: Cuboid,
    pub material: Material,
}

/// Lo que hay que trazar: objetos, luces, el color del vacio y el halo.
///
/// El bloom viaja con la escena y no con el renderer porque es parte de
/// **como se ve esta escena**, igual que el color del fondo: el teseracto
/// no se lee sin halo, y el cubo mate no tiene nada sobreexpuesto que
/// derramar. Dejarlo en el renderer obligaria a que cada sitio que dibuja
/// recuerde con que ajuste va cada escena.
pub struct Scene {
    pub objects: Vec<Object>,
    pub lights: Vec<Light>,
    pub background: Color,
    pub bloom: Bloom,
}

impl Scene {
    /// Impacto **mas cercano** contra la escena, junto con el objeto que se
    /// toco.
    ///
    /// Recorre todo y se queda con el menor `distance`. Quedarse con el
    /// primer impacto encontrado funcionaba con un solo cubo; con el
    /// teseracto dibujaria el nucleo por encima del cascaron segun el orden
    /// de la lista.
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

/// El cubo mate del primer paso: opaco, naranja, una luz y nada mas.
///
/// Se queda en el proyecto porque es la referencia contra la cual se lee el
/// teseracto, y porque es la escena donde la difusa se ve sola, sin nada
/// encima que la disimule.
///
/// La luz va muy arriba y adelantada, deliberadamente **desalineada** del
/// ojo: puesta detras de la camara, las tres caras visibles recibirian casi
/// la misma cantidad de luz y el cubo se veria como un hexagono plano.
pub fn cubito() -> Scene {
    Scene {
        objects: vec![Object {
            shape: Cuboid::cubo(Vec3::zeros(), LADO),
            material: albedo_naranja(),
        }],
        lights: vec![Light::blanca(Vec3::new(1.0, 4.0, 2.0) * LADO * 0.75, 1.25)],
        background: Color::from_hex(0x10141C),
        // Nada pasa del blanco en esta escena: no hay rango perdido que
        // insinuar con un halo.
        bloom: Bloom::apagado(),
    }
}

/// El teseracto: dos cubos concentricos, vidrio azul y un nucleo
/// incandescente.
///
/// **El cubo dentro del cubo no es decorado.** Es la proyeccion clasica del
/// hipercubo de cuatro dimensiones: al proyectar un teseracto a tres
/// dimensiones, la celda «lejana» en la cuarta queda dentro de la cercana.
/// Que la forma del enunciado sea un cuboide alineado a los ejes y que la
/// figura pedida sea un cubo dentro de otro es una coincidencia comoda: sin
/// rotaciones de por medio, dos AABB concentricos son exactamente la
/// proyeccion.
///
/// Lo que hace que se lea como el objeto de la pelicula son cuatro cosas, y
/// ninguna es un reflejo:
///
/// 1. **Absorcion tenida** en el cascaron. El azul sale porque el medio se
///    come el rojo; el degradado entre el centro y las esquinas sale de que
///    el rayo cruza mas vidrio por el centro.
/// 2. **Emision** en dos escalas: la del nucleo, muy por encima del
///    blanco, y la del **medio** del cascaron, repartida por unidad de
///    longitud. La segunda es la que llena la masa de luz.
/// 3. **Marco en las aristas** de los dos cubos, que es lo que dibuja la
///    jaula: las aristas del cubo interior se ven **a traves** del vidrio.
/// 4. **Halo**, que es lo unico que conserva alguna pista del rango que el
///    recorte a 8 bits tira.
/// 5. **El piso**, que es donde todo lo anterior se vuelve comprobable: el
///    charco de luz azul mide cuanta luz sale del objeto, y la sombra
///    —azul, con el nucleo recortado en oscuro— mide cuanta deja pasar.
///    Es tambien una losa, es decir un `Cuboid` aplastado: no hizo falta
///    primitiva nueva.
pub fn teseracto() -> Scene {
    let cascaron = Object {
        shape: Cuboid::cubo(Vec3::zeros(), LADO),
        material: Material::opaco(Color::from_hex(0x0E2A4D))
            // Casi todo pasa: el cascaron es vidrio, no plastico teido.
            // La absorcion se come el rojo y deja correr el azul, y por eso
            // el centro —mas vidrio que cruzar— sale mas saturado que las
            // esquinas.
            .traslucido(0.90, Color::new(1.25, 0.50, 0.16))
            // El resplandor del volumen: es lo que llena la masa de luz y
            // lo que hace que el teseracto se lea como un objeto solido
            // encendido y no como una jaula de alambre.
            .con_resplandor(Color::new(0.04, 0.17, 0.38))
            .con_emision(Color::new(0.02, 0.09, 0.20))
            .con_marco(Color::new(0.55, 1.70, 2.60), 0.045),
    };

    let nucleo = Object {
        shape: Cuboid::cubo(Vec3::zeros(), LADO * 0.42),
        // Opaco y muy por encima del blanco: es la fuente del resplandor.
        // El tinte cyan sobrevive al recorte solo en el halo, porque el
        // centro se empaca como blanco puro.
        material: Material::opaco(Color::from_hex(0x8FD8FF))
            .con_emision(Color::new(3.40, 4.80, 5.80))
            .con_marco(Color::new(4.20, 5.60, 6.40), 0.10),
    };

    // Gris medio y desaturado. Un piso con color propio competiria con el
    // charco azul, pero uno **oscuro** tampoco sirve: un 0x2C en sRGB son
    // apenas 0.026 de energia lineal, y por mucha luz que reciba no
    // devuelve casi nada. Lo que se busca es una superficie sin caracter
    // que deje ver la luz que le llega.
    let piso = Object {
        shape: Cuboid::centrado(
            Vec3::new(0.0, ALTURA_DEL_PISO - GROSOR_DEL_PISO * 0.5, 0.0),
            Vec3::new(EXTENSION_DEL_PISO, GROSOR_DEL_PISO, EXTENSION_DEL_PISO),
        ),
        material: Material::opaco(Color::from_hex(0x5A6474)),
    };

    Scene {
        // El orden no le importa a `cast`, que se queda con el impacto mas
        // cercano; importa para los indices de las pruebas.
        objects: vec![cascaron, nucleo, piso],
        lights: vec![
            // Clave: fria, muy arriba y **detras** del objeto respecto del
            // encuadre inicial. Es la unica que proyecta sombra, y de donde
            // este depende que la sombra se vea: puesta del lado de la
            // camara, la mancha cae detras del teseracto y el propio
            // teseracto la tapa. Desde atras, cae hacia el frente.
            Light::blanca(Vec3::new(-1.2, 3.2, -1.6) * LADO * 0.8, 2.60)
                .con_color(Color::from_hex(0xBFE0FF)),
            // La del propio teseracto. Nace en su centro y no proyecta
            // sombra: no es una lampara escondida en la caja, es el resumen
            // de la luz que escapa de todo el volumen. Ver
            // `Light::casts_shadows`. Es la que pinta el charco azul.
            Light::blanca(Vec3::zeros(), 4.50)
                .con_color(Color::from_hex(0x5BC8FF))
                .con_distancia_de_referencia(LADO)
                .sin_sombras(),
        ],
        background: Color::from_hex(0x04060C),
        bloom: Bloom::teseracto(),
    }
}

/// Naranja calido del cubo mate: contrasta con el fondo frio y deja ver
/// bien el degradado entre caras.
fn albedo_naranja() -> Material {
    Material::opaco(Color::from_hex(0xE2703A))
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
    // Un palmo sobre el piso: orbitar hacia abajo se frena al ras en vez
    // de meter la camara bajo la losa y llenar la pantalla de negro. El
    // tope se aplica tambien en la escena del cubo mate, que comparte
    // camara; ahi no hay piso que estorbe, y perder la vista desde
    // exactamente abajo de un cubo no cuesta nada.
    .with_min_eye_height(ALTURA_DEL_PISO + LADO * 0.1)
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
            material: Material::opaco(Color::new(1.0, 0.0, 0.0)),
        };
        let cercano = Object {
            shape: Cuboid::cubo(Vec3::zeros(), 1.0),
            material: Material::opaco(Color::new(0.0, 1.0, 0.0)),
        };
        let scene = Scene {
            objects: vec![lejano, cercano],
            lights: vec![],
            background: Color::black(),
            bloom: Bloom::apagado(),
        };

        let ray = Ray::new(Vec3::new(0.0, 0.0, 5.0), Vec3::new(0.0, 0.0, -1.0));
        let (hit, objeto) = scene.cast(&ray).expect("debe impactar");

        assert_eq!(objeto.material.albedo, cercano.material.albedo);
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

    #[test]
    fn el_nucleo_del_teseracto_cabe_dentro_del_cascaron() {
        // Si no cupiera, asomaria por las caras y dejaria de leerse como un
        // cubo dentro de otro.
        let scene = teseracto();
        let cascaron = scene.objects[0].shape.bounds;
        let nucleo = scene.objects[1].shape.bounds;

        for eje in 0..3 {
            assert!(nucleo.min[eje] > cascaron.min[eje], "eje {eje}");
            assert!(nucleo.max[eje] < cascaron.max[eje], "eje {eje}");
        }
    }

    #[test]
    fn el_rayo_central_del_teseracto_entra_por_el_cascaron() {
        // El orden importa: lo primero que se toca es el vidrio, y el
        // nucleo solo aparece despues de atravesarlo.
        let scene = teseracto();
        let camera = camara_inicial();

        let (_, objeto) = scene
            .cast(&camera.ray_from_pixel(400, 300, 800, 600))
            .expect("el teseracto esta encuadrado");

        assert!(objeto.material.transmission > 0.0, "toco el cascaron");
    }

    #[test]
    fn el_nucleo_emite_mas_que_el_cascaron() {
        let scene = teseracto();

        let cascaron = scene.objects[0].material.emission;
        let nucleo = scene.objects[1].material.emission;

        assert!(
            nucleo.b > cascaron.b * 10.0,
            "{} contra {}",
            nucleo.b,
            cascaron.b
        );
        assert!(nucleo.b > 1.0, "el nucleo tiene que pasar del blanco");
    }

    #[test]
    fn el_piso_queda_debajo_del_teseracto_sin_tocarlo() {
        // Flotando, no apoyado: la sombra se despega y se lee como sombra.
        let scene = teseracto();
        let cascaron = scene.objects[0].shape.bounds;
        let piso = scene.objects[2].shape.bounds;

        assert!(piso.max.y < cascaron.min.y, "el piso toca el teseracto");
        assert!(
            (piso.max.y - ALTURA_DEL_PISO).abs() < 1e-5,
            "la cara superior deberia estar en ALTURA_DEL_PISO"
        );
    }

    #[test]
    fn el_piso_es_mucho_mas_ancho_que_el_objeto() {
        // Lo bastante para que su borde quede fuera del alcance de las
        // luces y se desvanezca en negro en vez de cortarse en una linea.
        let scene = teseracto();
        let piso = scene.objects[2].shape.bounds;

        assert!(piso.max.x - piso.min.x > LADO * 20.0);
        assert!(piso.max.z - piso.min.z > LADO * 20.0);
    }

    #[test]
    fn un_rayo_hacia_abajo_encuentra_el_piso() {
        let scene = teseracto();
        // Al lado del teseracto, para no tocarlo de paso.
        let ray = Ray::new(Vec3::new(4.0, 5.0, 4.0), Vec3::new(0.0, -1.0, 0.0));

        let (hit, _) = scene.cast(&ray).expect("el piso esta ahi abajo");

        assert!(
            (hit.point.y - ALTURA_DEL_PISO).abs() < 1e-4,
            "{:?}",
            hit.point
        );
        assert_eq!(hit.normal, Vec3::new(0.0, 1.0, 0.0));
    }

    #[test]
    fn solo_una_luz_proyecta_sombra() {
        // La otra nace dentro del nucleo opaco: si proyectara sombra, el
        // piso quedaria negro justo debajo de lo que lo ilumina.
        let scene = teseracto();

        let con_sombra = scene.lights.iter().filter(|luz| luz.casts_shadows).count();

        assert_eq!(con_sombra, 1, "de {} luces", scene.lights.len());
    }

    #[test]
    fn la_luz_interna_nace_en_el_centro_del_teseracto() {
        let scene = teseracto();
        let interna = scene
            .lights
            .iter()
            .find(|luz| !luz.casts_shadows)
            .expect("la luz del propio teseracto");

        assert!(
            interna.position.magnitude() < 1e-5,
            "{:?}",
            interna.position
        );
        // Sin una referencia explicita, una luz en el origen no tendria de
        // donde sacar su escala de distancia.
        assert!(interna.reference_distance > 0.5);
    }

    #[test]
    fn la_clave_esta_del_lado_opuesto_a_la_camara() {
        // De esto depende que la sombra se vea: con la luz del lado del
        // ojo, la mancha cae detras del objeto y el objeto la tapa.
        let scene = teseracto();
        let camera = camara_inicial();
        let clave = scene
            .lights
            .iter()
            .find(|luz| luz.casts_shadows)
            .expect("la luz clave");

        let hacia_luz = clave.position.normalize();
        let hacia_ojo = camera.eye.normalize();

        assert!(
            nalgebra_glm::dot(&hacia_luz, &hacia_ojo) < 0.0,
            "la luz deberia estar detras del objeto"
        );
    }

    #[test]
    fn el_cascaron_absorbe_mas_rojo_que_azul() {
        // Es de donde sale el azul: no del albedo, sino de lo que el medio
        // se come.
        let absorcion = teseracto().objects[0].material.absorption;

        assert!(absorcion.r > absorcion.b * 3.0, "{absorcion:?}");
    }
}
