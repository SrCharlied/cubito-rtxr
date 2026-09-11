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

/// Un cubo mate sobre un piso: opaco, naranja, una luz y **nada mas**.
///
/// Es el caso minimo del enunciado —un cubo, luz difusa— y se queda en el
/// proyecto por dos razones, aunque la ventana ya no lo muestre: es la
/// escena mas simple donde la difusa se ve sola, y varias pruebas se
/// apoyan en eso. Sigue disponible en `render_ppm --escena cubo`.
///
/// La luz va muy arriba y adelantada, deliberadamente **desalineada** del
/// ojo: puesta detras de la camara, las tres caras visibles recibirian casi
/// la misma cantidad de luz y el cubo se veria como un hexagono plano.
///
/// El cubo **flota** sobre el piso, igual que las otras dos escenas.
/// Apoyado, su sombra nace debajo de el y queda escondida por el propio
/// cubo justo donde se la quiere ver.
pub fn cubito() -> Scene {
    Scene {
        objects: vec![
            Object {
                shape: Cuboid::cubo(Vec3::zeros(), LADO),
                material: albedo_naranja(),
            },
            // Piso frio y apagado: el contraste con el naranja separa el
            // objeto del suelo sin que el suelo pida atencion.
            losa(Color::from_hex(0x3E4657)),
        ],
        lights: vec![Light::blanca(Vec3::new(1.0, 4.0, 2.0) * LADO * 0.75, 1.25)],
        background: Color::from_hex(0x10141C),
        // Nada pasa del blanco en esta escena: no hay rango perdido que
        // insinuar con un halo.
        bloom: Bloom::apagado(),
    }
}

/// Grosor de las barras de la jaula, como fraccion del lado. Lo bastante
/// finas para que se vea el nucleo por los huecos, lo bastante gruesas
/// para que cada una tenga volumen propio y se sombreen entre si.
const GROSOR_BARRA: f32 = LADO * 0.09;

/// El teseracto en **luz difusa pura**: la misma figura, sin una sola gota
/// de emision, transmision ni halo.
///
/// Es la escena del enunciado, y nace de una pregunta: si a un teseracto
/// se le quita la transmision, ¿que queda? Con un cascaron solido, nada:
/// un cubo opaco dentro de otro cubo opaco **no se ve**, y el resultado es
/// un hexagono liso. Lo que se pierde al apagar el vidrio no es el brillo,
/// es la estructura.
///
/// La salida es dejar de pedirle al material lo que puede dar la
/// geometria. Aqui el cubo exterior no es una caja sino **doce barras**
/// sobre sus aristas, y el nucleo se ve por los huecos. Ninguna
/// transmision de por medio: es una jaula, y lo que hay dentro se ve
/// porque la jaula esta abierta.
///
/// El precio es que se lee como un modelo fisico —una escultura de
/// varillas con una pieza maciza dentro— y no como el objeto de la
/// pelicula. Es un precio justo: a cambio, todo lo que decide el color de
/// un pixel en esta escena es la ley de Lambert y si algo se interpone.
pub fn jaula() -> Scene {
    let mut objects = Vec::with_capacity(14);
    let marco = Material::opaco(Color::from_hex(0xE2703A));
    let medio = LADO * 0.5;
    // Las barras se estiran un grosor de mas para que lleguen al plano
    // exterior de la esquina. Se traslapan de a tres ahi, y ese traslape es
    // justo lo que dibuja una esquina cuadrada en vez de tres puntas
    // sueltas. Que se solapen no le molesta a nadie: `cast` se queda con el
    // impacto mas cercano.
    let largo = LADO + GROSOR_BARRA;

    // Las doce aristas son cuatro por eje: la barra se estira sobre su eje
    // y queda fina en los otros dos, y las cuatro copias salen de los
    // cuatro signos de esas dos coordenadas.
    for eje in 0..3 {
        let (otro_a, otro_b) = ((eje + 1) % 3, (eje + 2) % 3);

        for (signo_a, signo_b) in [(-1.0, -1.0), (-1.0, 1.0), (1.0, -1.0), (1.0, 1.0)] {
            let mut centro = Vec3::zeros();
            let mut tamano = Vec3::new(GROSOR_BARRA, GROSOR_BARRA, GROSOR_BARRA);

            tamano[eje] = largo;
            centro[otro_a] = signo_a * medio;
            centro[otro_b] = signo_b * medio;

            objects.push(Object {
                shape: Cuboid::centrado(centro, tamano),
                material: marco,
            });
        }
    }

    // El nucleo, del mismo tamano que el del teseracto: hueso contra cobre,
    // que bajo luz difusa separa las dos piezas sin necesidad de brillo.
    objects.push(Object {
        shape: Cuboid::cubo(Vec3::zeros(), LADO * 0.42),
        material: Material::opaco(Color::from_hex(0xE8DCC8)),
    });
    objects.push(losa(Color::from_hex(0x3E4657)));

    Scene {
        objects,
        // Dos luces, y las dos proyectan sombra. Varias luces siguen siendo
        // luz difusa: lo que define a la difusa es la ley de Lambert, no
        // cuantas lamparas hay. Con una sola, la mitad de las barras cae a
        // puro ambiente y la jaula se lee plana; con dos de temperaturas
        // distintas, cada barra recibe algo por los dos lados y las sombras
        // se cruzan con tintes distintos.
        lights: vec![
            // Clave calida, arriba y del lado del ojo.
            Light::blanca(Vec3::new(1.0, 4.0, 2.0) * LADO * 0.75, 1.25)
                .con_color(Color::from_hex(0xFFE8D0)),
            // Relleno frio y bajo, desde el lado contrario.
            Light::blanca(Vec3::new(-2.2, 1.4, -1.8) * LADO * 0.7, 0.55)
                .con_color(Color::from_hex(0x9FC4FF)),
        ],
        background: Color::from_hex(0x10141C),
        // Nada pasa del blanco en esta escena: no hay rango perdido que
        // insinuar con un halo.
        bloom: Bloom::apagado(),
    }
}

/// La losa que hace de piso, con el albedo que se le pase.
///
/// Es un `Cuboid` aplastado: la primitiva del proyecto alcanza, no hizo
/// falta un plano. Las dos escenas comparten altura y extension para que
/// el tope de camara —que sale de `ALTURA_DEL_PISO`— valga para ambas.
fn losa(albedo: Color) -> Object {
    Object {
        shape: Cuboid::centrado(
            Vec3::new(0.0, ALTURA_DEL_PISO - GROSOR_DEL_PISO * 0.5, 0.0),
            Vec3::new(EXTENSION_DEL_PISO, GROSOR_DEL_PISO, EXTENSION_DEL_PISO),
        ),
        material: Material::opaco(albedo),
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
    let piso = losa(Color::from_hex(0x5A6474));

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
        let cubo = scene.objects[0].material.albedo;
        let mut vistas: Vec<Vec3> = Vec::new();

        for y in (0..600).step_by(20) {
            for x in (0..800).step_by(20) {
                let Some((hit, objeto)) = scene.cast(&camera.ray_from_pixel(x, y, 800, 600)) else {
                    continue;
                };

                // El piso mira hacia arriba igual que la cara de arriba del
                // cubo: contarlo inflaria la cuenta de caras vistas.
                if objeto.material.albedo != cubo {
                    continue;
                }

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
    fn la_jaula_son_doce_barras_un_nucleo_y_un_piso() {
        assert_eq!(jaula().objects.len(), 14);
    }

    #[test]
    fn cada_barra_es_larga_en_un_solo_eje() {
        // Una barra que fuera larga en dos ejes seria una placa, y la jaula
        // dejaria de dejar ver el nucleo.
        let scene = jaula();
        let largo = LADO + GROSOR_BARRA;

        for (i, barra) in scene.objects.iter().take(12).enumerate() {
            let bounds = barra.shape.bounds;
            let tamano = bounds.max - bounds.min;

            let largos = (0..3).filter(|&e| (tamano[e] - largo).abs() < 1e-5).count();
            let finos = (0..3)
                .filter(|&e| (tamano[e] - GROSOR_BARRA).abs() < 1e-5)
                .count();

            assert_eq!(largos, 1, "barra {i}: {tamano:?}");
            assert_eq!(finos, 2, "barra {i}: {tamano:?}");
        }
    }

    #[test]
    fn las_doce_barras_ocupan_aristas_distintas() {
        let scene = jaula();
        let mut centros: Vec<Vec3> = Vec::new();

        for barra in scene.objects.iter().take(12) {
            let centro = barra.shape.bounds.centro();

            assert!(
                !centros.iter().any(|c| (c - centro).magnitude() < 1e-5),
                "dos barras en la misma arista: {centro:?}"
            );
            centros.push(centro);
        }
    }

    #[test]
    fn la_jaula_encierra_el_mismo_volumen_que_el_teseracto() {
        // Las barras tienen que delimitar el cubo de lado LADO, no una caja
        // de otro tamano: es lo que hace comparables las dos escenas al
        // alternar entre ellas sin mover la camara.
        let scene = jaula();
        let mut minimo = Vec3::new(f32::MAX, f32::MAX, f32::MAX);
        let mut maximo = Vec3::new(f32::MIN, f32::MIN, f32::MIN);

        for barra in scene.objects.iter().take(12) {
            for eje in 0..3 {
                minimo[eje] = minimo[eje].min(barra.shape.bounds.min[eje]);
                maximo[eje] = maximo[eje].max(barra.shape.bounds.max[eje]);
            }
        }

        // El cubo de lado LADO, crecido medio grosor por lado: es lo que
        // sobresale al estirar las barras para cuadrar las esquinas.
        let esperado = LADO * 0.5 + GROSOR_BARRA * 0.5;

        for eje in 0..3 {
            assert!(
                (minimo[eje] + esperado).abs() < 1e-5,
                "eje {eje}: {minimo:?}"
            );
            assert!(
                (maximo[eje] - esperado).abs() < 1e-5,
                "eje {eje}: {maximo:?}"
            );
        }
    }

    #[test]
    fn el_nucleo_de_la_jaula_cabe_entre_las_barras() {
        // Si tocara las barras dejaria de leerse como una pieza suelta
        // dentro de la jaula.
        let scene = jaula();
        let nucleo = scene.objects[12].shape.bounds;
        let hueco = LADO * 0.5 - GROSOR_BARRA;

        for eje in 0..3 {
            assert!(nucleo.max[eje] < hueco, "eje {eje}: {nucleo:?}");
            assert!(nucleo.min[eje] > -hueco, "eje {eje}: {nucleo:?}");
        }
    }

    #[test]
    fn la_jaula_tiene_dos_luces_y_las_dos_proyectan_sombra() {
        // Varias luces siguen siendo luz difusa: lo que define a la difusa
        // es la ley de Lambert, no cuantas lamparas hay.
        let scene = jaula();

        assert_eq!(scene.lights.len(), 2);
        assert!(scene.lights.iter().all(|luz| luz.casts_shadows));
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
