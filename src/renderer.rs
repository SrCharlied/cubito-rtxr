use crate::bloom;
use crate::camera::Camera;
use crate::color::Color;
use crate::framebuffer::Framebuffer;
use crate::hit::Hit;
use crate::light::{direct_diffuse, AMBIENT};
use crate::material::{beer, Material};
use crate::ray::Ray;
use crate::scene::{Object, Scene};
use crate::EPSILON;
use nalgebra_glm::Vec3;

/// Cuantos cascarones puede atravesar un rayo antes de rendirse.
///
/// El teseracto necesita tres —entrar al vidrio, tocar el nucleo, y en los
/// rayos que lo esquivan salir por la cara de atras—. Seis deja margen para
/// agregar una capa mas sin tocar esto, y acota el costo: sin un techo, dos
/// superficies transmisivas mal puestas se trazarian entre si para siempre.
const MAX_TRANSMISSION_DEPTH: usize = 6;

/// Que se pinta en cada pixel.
///
/// Los tres modos son los tres escalones de construccion del proyecto, y se
/// dejan disponibles porque siguen siendo las herramientas de diagnostico:
/// cuando la imagen sombreada se ve mal, `Normals` dice si el problema esta
/// en la geometria o en la luz.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shading {
    /// Raycaster puro: la normal de la cara, llevada a color. Sin luces de
    /// por medio, cada cara sale de un color distinto y fijo.
    Normals,
    /// Solo el color propio del objeto. Silueta correcta, cero volumen.
    Albedo,
    /// Ambiente, difusa, emision y transmision. Es el modo de la entrega.
    Diffuse,
}

impl Shading {
    /// Solo el modo de la entrega lleva post-proceso. Los de diagnostico
    /// existen para mostrar el dato crudo, y el halo taparia justo eso.
    fn lleva_post_proceso(self) -> bool {
        self == Shading::Diffuse
    }
}

/// Traza un rayo primario y devuelve el color que le corresponde.
pub fn trace(scene: &Scene, ray: &Ray, shading: Shading) -> Color {
    traza(scene, ray, shading, 0).0
}

/// El trazado con profundidad, que es lo que la transmision necesita y el
/// llamador no tiene por que conocer.
///
/// Devuelve tambien **hasta donde llego** el rayo, y esa segunda mitad no
/// es un detalle: es lo que le permite a quien mira desde dentro de un
/// medio saber cuanto medio hubo de verdad entre su cara y lo que hay
/// detras. Sin ese dato habria que cobrar la cuerda entera del volumen
/// incluso cuando algo la interrumpe a mitad de camino, y el nucleo del
/// teseracto —que es justo eso, algo que la interrumpe— saldria mucho mas
/// oscuro y mas tenido de lo que le toca.
///
/// `f32::INFINITY` cuando el rayo se va al fondo sin tocar nada.
fn traza(scene: &Scene, ray: &Ray, shading: Shading, depth: usize) -> (Color, f32) {
    let Some((hit, objeto)) = scene.cast(ray) else {
        return (scene.background, f32::INFINITY);
    };

    let color = match shading {
        Shading::Normals => color_por_normal(&hit),
        Shading::Albedo => objeto.material.albedo,
        Shading::Diffuse => sombrear(scene, ray, &hit, objeto, shading, depth),
    };

    (color, hit.distance)
}

/// El color de una superficie: lo que refleja, lo que emite y lo que deja
/// pasar.
///
/// Los tres terminos se combinan de distinta forma a proposito:
///
/// - Lo **difuso** y lo **transmitido** se reparten segun `transmission`:
///   son la misma luz vista de dos maneras, y sumar las dos enteras
///   devolveria mas luz de la que llego.
/// - La **emision** y el **marco** se suman aparte, sin pesar. Son luz
///   propia, no luz reflejada: atenuar el marco al ocho por ciento en un
///   cascaron casi transparente apagaria justo lo que dibuja la jaula.
fn sombrear(
    scene: &Scene,
    ray: &Ray,
    hit: &Hit,
    objeto: &Object,
    shading: Shading,
    depth: usize,
) -> Color {
    let material = &objeto.material;
    let superficie = material.emission + material.edge_emission(&hit.uv);
    let reflejado = difusa(scene, hit, material.albedo);

    if material.transmission <= 0.0 || depth >= MAX_TRANSMISSION_DEPTH {
        return reflejado + superficie;
    }

    // La cuerda completa del volumen, que es el techo de lo que el rayo
    // puede recorrer aqui dentro. Solo se mide **al entrar**: al salir, el
    // tramo ya se pago.
    let cuerda = if hit.front_face {
        objeto.shape.thickness(ray)
    } else {
        0.0
    };

    let (detras, camino) = atravesar(scene, ray, hit, material, shading, depth, cuerda);

    reflejado * (1.0 - material.transmission)
        + detras * material.transmission
        + superficie
        + material.inner_glow * camino
}

/// Lo que se ve **a traves** de esta superficie, ya tenido por el medio.
///
/// El rayo sigue derecho: no hay refraccion. Doblarlo exige Snell mas
/// Fresnel para repartir la energia entre lo que entra y lo que se refleja,
/// y sin reflexiones que acompanen, un cubo que deforma lo de atras pero no
/// devuelve nada del entorno se ve mas raro que uno que no deforma. El
/// tenido volumetrico es el que hace el trabajo aqui.
///
/// Devuelve el color de atras y **cuanto medio hubo que cruzar** para
/// llegar a el, que es lo que despues alimenta al resplandor del volumen.
///
/// Ese camino es lo que el rayo recorre hasta lo siguiente que encuentre,
/// acotado por `cuerda`. Las dos mitades del acotamiento importan:
///
/// - Si algo interrumpe antes de la cara de salida —el nucleo—, se cobra
///   solo hasta ahi. Cobrar la cuerda entera tenia el nucleo como si la
///   luz hubiera cruzado tambien el vidrio que tiene **detras**, que es
///   vidrio que nunca atraveso.
/// - Si no lo interrumpe nada, el rayo sale por la cara opuesta y sigue
///   hasta el fondo. Ahi el recorrido es infinito y la cuerda es el unico
///   tope sensato: fuera del volumen ya no hay medio que cobrar.
fn atravesar(
    scene: &Scene,
    ray: &Ray,
    hit: &Hit,
    material: &Material,
    shading: Shading,
    depth: usize,
    cuerda: f32,
) -> (Color, f32) {
    // Despegar el origen de la cara que se acaba de tocar. Sin el margen,
    // el error de redondeo puede dejar el punto del lado de afuera y el
    // rayo vuelve a impactar la misma cara, con el volumen entero perdido.
    let continuacion = Ray::new(hit.point + ray.direction * EPSILON, ray.direction);

    let (detras, recorrido) = traza(scene, &continuacion, shading, depth + 1);
    let camino = recorrido.min(cuerda);

    (detras * beer(material.absorption, camino), camino)
}

/// Ambiente mas el aporte difuso de cada luz.
///
/// El ambiente no es fisica: es el piso que conserva la silueta de lo que
/// ninguna luz alcanza. Cuenta como color propio del objeto, asi que se
/// tine con el albedo igual que la difusa.
fn difusa(scene: &Scene, hit: &Hit, albedo: Color) -> Color {
    let mut color = albedo * AMBIENT;

    for light in &scene.lights {
        let hacia_luz = light.position - hit.point;
        let distancia = hacia_luz.magnitude();

        // Luz exactamente sobre la superficie: no hay direccion que
        // normalizar y el aporte seria una division entre cero.
        if distancia <= f32::EPSILON {
            continue;
        }

        let atenuacion = light.attenuation(distancia);
        if atenuacion <= 0.0 {
            continue;
        }

        let direccion = hacia_luz / distancia;

        // Cuanta de esta luz sobrevive el camino hasta aqui. Es un color y
        // no una bandera: la sombra del teseracto es azul porque lo que la
        // proyecta es vidrio azul.
        let sombra = if light.casts_shadows {
            transmitancia(scene, &hit.point, &direccion, distancia, 0)
        } else {
            Color::white()
        };

        color = color
            + direct_diffuse(albedo, &hit.normal, &direccion, light.color, atenuacion) * sombra;
    }

    color
}

/// Que fraccion de la luz sobrevive el trayecto de `origen` a la luz, por
/// canal.
///
/// Blanco si no hay nada en medio, negro si hay algo opaco, y un color si
/// lo que hay transmite: cada cascaron cobra su transmision y la absorcion
/// del tramo que se cruza, igual que en el camino de la camara. De ahi sale
/// que la sombra del teseracto no sea un parche gris sino una mancha azul
/// con el nucleo recortado en oscuro.
///
/// La recursion se corta con el mismo techo que la transmision, y al
/// llegar a el trata lo que quede como opaco. Devolver blanco ahi seria
/// peor: dejaria pasar luz entera a traves de una pila de cascarones que
/// justamente no se pudo resolver.
fn transmitancia(
    scene: &Scene,
    origen: &Vec3,
    hacia_luz: &Vec3,
    distancia: f32,
    depth: usize,
) -> Color {
    if depth >= MAX_TRANSMISSION_DEPTH {
        return Color::black();
    }

    // El margen despega el rayo de la superficie que lo origina. Sin el,
    // cada punto se hace sombra a si mismo y la escena entera sale sucia de
    // motas oscuras: es el acne de sombras.
    let ray = Ray::new(origen + hacia_luz * EPSILON, *hacia_luz);

    let Some((hit, objeto)) = scene.cast(&ray) else {
        return Color::white();
    };

    // Un objeto **detras** de la luz no tapa nada.
    if hit.distance >= distancia - EPSILON {
        return Color::white();
    }

    let material = &objeto.material;
    if material.transmission <= 0.0 {
        return Color::black();
    }

    let cuerda = if hit.front_face {
        objeto.shape.thickness(&ray)
    } else {
        0.0
    };
    let camino = cuerda.min(distancia - hit.distance);

    let mas_alla = transmitancia(
        scene,
        &hit.point,
        hacia_luz,
        distancia - hit.distance,
        depth + 1,
    );

    beer(material.absorption, camino) * material.transmission * mas_alla
}

/// Normal a color, con el mapeo habitual de -1..1 a 0..1.
///
/// El valor que sale de aqui es un **dato**, no una medida de luz, asi que
/// se construye con `Color::new` y no con `from_hex`: pasarlo por la
/// decodificacion sRGB lo distorsionaria y las caras dejarian de leerse
/// como lo que son.
fn color_por_normal(hit: &Hit) -> Color {
    Color::new(
        hit.normal.x * 0.5 + 0.5,
        hit.normal.y * 0.5 + 0.5,
        hit.normal.z * 0.5 + 0.5,
    )
}

/// Dibuja la escena completa y deja el framebuffer listo para presentar.
///
/// Tres pasos, y el orden es el que importa: trazar a rango completo,
/// derramar el halo sobre lo que paso del blanco, y solo entonces empacar.
/// El bloom despues del recorte no tendria de donde sacar la informacion.
///
/// Un rayo por pixel, sin antialiasing: las aristas salen duras. Es lo que
/// corresponde a este paso; suavizarlas es muestrear varias veces por pixel
/// y eso pertenece a otro.
pub fn render(framebuffer: &mut Framebuffer, scene: &Scene, camera: &Camera, shading: Shading) {
    let (width, height) = (framebuffer.width, framebuffer.height);

    for y in 0..height {
        for x in 0..width {
            let ray = camera.ray_from_pixel(x, y, width, height);

            framebuffer.set(x, y, trace(scene, &ray, shading));
        }
    }

    if shading.lleva_post_proceso() {
        bloom::apply(&mut framebuffer.hdr, width, height, &scene.bloom);
    }

    framebuffer.pack();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{camara_inicial, cubito, teseracto, ALTURA_DEL_PISO};
    use nalgebra_glm::Vec3;

    /// Resolucion chica: estas pruebas verifican relaciones entre pixeles,
    /// no la imagen final, y a 80 x 60 corren en un parpadeo.
    const ANCHO: usize = 80;
    const ALTO: usize = 60;
    const CENTRO: usize = (ALTO / 2) * ANCHO + ANCHO / 2;

    fn render_de(scene: &Scene, shading: Shading) -> Framebuffer {
        let mut framebuffer = Framebuffer::new(ANCHO, ALTO);
        render(&mut framebuffer, scene, &camara_inicial(), shading);

        framebuffer
    }

    #[test]
    fn el_fondo_queda_donde_no_hay_cubo() {
        let framebuffer = render_de(&cubito(), Shading::Diffuse);
        let fondo = cubito().background.to_u32();

        assert_eq!(framebuffer.buffer[0], fondo, "esquina superior izquierda");
        assert_eq!(
            framebuffer.buffer[ANCHO * ALTO - 1],
            fondo,
            "esquina inferior derecha"
        );
    }

    #[test]
    fn el_centro_no_es_el_fondo() {
        let framebuffer = render_de(&cubito(), Shading::Diffuse);

        assert_ne!(framebuffer.buffer[CENTRO], cubito().background.to_u32());
    }

    #[test]
    fn las_tres_caras_visibles_salen_con_brillos_distintos() {
        // Es la prueba de la difusa: sin ella, las tres caras tendrian
        // exactamente el mismo color y el cubo se veria como un hexagono
        // plano.
        let scene = cubito();
        let camera = camara_inicial();
        let mut por_cara: Vec<(Vec3, f32)> = Vec::new();

        for y in 0..ALTO {
            for x in 0..ANCHO {
                let ray = camera.ray_from_pixel(x, y, ANCHO, ALTO);
                let Some((hit, objeto)) = scene.cast(&ray) else {
                    continue;
                };

                if por_cara
                    .iter()
                    .any(|(n, _)| (n - hit.normal).magnitude() < 1e-4)
                {
                    continue;
                }

                let color = super::difusa(&scene, &hit, objeto.material.albedo);
                por_cara.push((hit.normal, color.r));
            }
        }

        assert_eq!(por_cara.len(), 3, "caras vistas: {por_cara:?}");

        for (i, (_, brillo)) in por_cara.iter().enumerate() {
            for (_, otro) in por_cara.iter().skip(i + 1) {
                assert!(
                    (brillo - otro).abs() > 0.02,
                    "dos caras con el mismo brillo: {brillo} y {otro}"
                );
            }
        }
    }

    #[test]
    fn ninguna_cara_cae_por_debajo_del_ambiente() {
        // El ambiente es justamente el piso: ni la cara peor orientada
        // puede quedar en negro y perder su silueta.
        let scene = cubito();
        let camera = camara_inicial();

        for y in 0..ALTO {
            for x in 0..ANCHO {
                let ray = camera.ray_from_pixel(x, y, ANCHO, ALTO);
                let Some((hit, objeto)) = scene.cast(&ray) else {
                    continue;
                };

                let color = super::difusa(&scene, &hit, objeto.material.albedo);
                let piso = objeto.material.albedo.r * AMBIENT;

                assert!(color.r >= piso - 1e-6, "{} bajo el piso {piso}", color.r);
            }
        }
    }

    #[test]
    fn orbitar_cambia_la_imagen() {
        let scene = cubito();
        let mut camera = camara_inicial();

        let mut antes = Framebuffer::new(ANCHO, ALTO);
        render(&mut antes, &scene, &camera, Shading::Diffuse);

        camera.orbit(std::f32::consts::PI / 4.0, 0.0);

        let mut despues = Framebuffer::new(ANCHO, ALTO);
        render(&mut despues, &scene, &camera, Shading::Diffuse);

        assert_ne!(antes.buffer, despues.buffer);
    }

    #[test]
    fn la_orbita_completa_vuelve_a_la_misma_imagen() {
        // Doce pasos de 30 grados son una vuelta entera: la imagen tiene
        // que reproducirse. Es lo que atrapa una deriva en el paso a
        // esfericas y de vuelta.
        let scene = cubito();
        let mut camera = camara_inicial();

        let mut antes = Framebuffer::new(ANCHO, ALTO);
        render(&mut antes, &scene, &camera, Shading::Diffuse);

        for _ in 0..12 {
            camera.orbit(std::f32::consts::PI / 6.0, 0.0);
        }

        let mut despues = Framebuffer::new(ANCHO, ALTO);
        render(&mut despues, &scene, &camera, Shading::Diffuse);

        assert_eq!(antes.buffer, despues.buffer);
    }

    #[test]
    fn el_modo_albedo_pinta_el_cubo_de_un_solo_color() {
        let framebuffer = render_de(&cubito(), Shading::Albedo);
        let scene = cubito();

        let fondo = scene.background.to_u32();
        let albedo = scene.objects[0].material.albedo.to_u32();

        assert!(framebuffer
            .buffer
            .iter()
            .all(|&pixel| pixel == fondo || pixel == albedo));
    }

    // ------------------------------------------------------- teseracto

    #[test]
    fn el_nucleo_se_ve_a_traves_del_cascaron() {
        // La prueba de la transmision: el centro de la imagen tiene que
        // salir mucho mas brillante que un punto del cascaron lejos del
        // nucleo, y eso solo puede venir de atravesar el vidrio.
        let scene = teseracto();
        let camera = camara_inicial();

        let centro = super::trace(
            &scene,
            &camera.ray_from_pixel(ANCHO / 2, ALTO / 2, ANCHO, ALTO),
            Shading::Diffuse,
        );

        // Un punto que si toca el cascaron pero no el nucleo: cerca del
        // borde de la silueta.
        let (x, y) = borde_del_cascaron(&scene, &camera);
        let orilla = super::trace(
            &scene,
            &camera.ray_from_pixel(x, y, ANCHO, ALTO),
            Shading::Diffuse,
        );

        assert!(
            centro.b > orilla.b * 3.0,
            "{} contra {}",
            centro.b,
            orilla.b
        );
        assert!(centro.b > 1.0, "el nucleo tiene que pasar del blanco");
    }

    #[test]
    fn el_cascaron_sale_azul_y_no_del_albedo() {
        // El azul viene de la absorcion, no del color propio: el canal azul
        // del pixel supera al rojo mucho mas de lo que lo hace el albedo.
        let scene = teseracto();
        let camera = camara_inicial();

        let (x, y) = borde_del_cascaron(&scene, &camera);
        let pixel = super::trace(
            &scene,
            &camera.ray_from_pixel(x, y, ANCHO, ALTO),
            Shading::Diffuse,
        );

        assert!(pixel.b > pixel.r * 2.0, "{pixel:?}");
    }

    #[test]
    fn el_marco_brilla_mas_que_el_centro_de_la_cara() {
        let scene = teseracto();
        let cascaron = &scene.objects[0].material;

        let esquina = cascaron.edge_emission(&nalgebra_glm::Vec2::new(0.0, 0.5));
        let medio = cascaron.edge_emission(&nalgebra_glm::Vec2::new(0.5, 0.5));

        assert!(esquina.b > medio.b, "{} contra {}", esquina.b, medio.b);
        assert_eq!(medio, Color::black());
    }

    #[test]
    fn el_halo_se_derrama_fuera_de_la_silueta() {
        // Con bloom, un pixel de fondo pegado al teseracto tiene que salir
        // mas claro que el mismo pixel sin bloom. Es lo que hace que el
        // objeto se vea encendido y no solo pintado.
        let scene = teseracto();
        let mut sin_halo = teseracto();
        sin_halo.bloom = crate::bloom::Bloom::apagado();

        let con = render_de(&scene, Shading::Diffuse);
        let sin = render_de(&sin_halo, Shading::Diffuse);

        let fondo = scene.background.to_u32();
        let derramados = con
            .buffer
            .iter()
            .zip(&sin.buffer)
            .filter(|(&c, &s)| s == fondo && c != fondo)
            .count();

        assert!(derramados > 100, "solo {derramados} pixeles con halo");
    }

    #[test]
    fn el_halo_no_toca_los_modos_de_diagnostico() {
        // `Normals` tiene que seguir mostrando exactamente seis colores
        // posibles mas el fondo, sin derrames ni transparencias.
        let framebuffer = render_de(&teseracto(), Shading::Normals);
        let mut distintos: Vec<u32> = framebuffer.buffer.clone();
        distintos.sort_unstable();
        distintos.dedup();

        assert!(
            distintos.len() <= 7,
            "{} colores distintos",
            distintos.len()
        );
    }

    #[test]
    fn la_recursion_termina_aunque_todo_sea_transparente() {
        // Un cascaron que transmite todo y no absorbe nada: el techo de
        // profundidad es lo unico que evita que esto no vuelva nunca.
        use crate::cuboid::Cuboid;
        use crate::material::Material;
        use crate::scene::Object;

        let transparente = Material::opaco(Color::white()).traslucido(1.0, Color::black());
        let scene = Scene {
            objects: (1..=5)
                .map(|i| Object {
                    shape: Cuboid::cubo(Vec3::zeros(), i as f32),
                    material: transparente,
                })
                .collect(),
            lights: vec![],
            background: Color::new(0.25, 0.5, 0.75),
            bloom: crate::bloom::Bloom::apagado(),
        };

        let ray = Ray::new(Vec3::new(0.0, 0.0, 9.0), Vec3::new(0.0, 0.0, -1.0));
        let color = super::trace(&scene, &ray, Shading::Diffuse);

        assert!(color.b.is_finite(), "{color:?}");
    }

    // ------------------------------------------------- piso y sombras

    #[test]
    fn nada_en_medio_deja_pasar_toda_la_luz() {
        let scene = teseracto();
        // Un punto muy arriba, mirando hacia arriba: no hay nada encima.
        let origen = Vec3::new(0.0, 40.0, 0.0);
        let arriba = Vec3::new(0.0, 1.0, 0.0);

        assert_eq!(
            super::transmitancia(&scene, &origen, &arriba, 10.0, 0),
            Color::white()
        );
    }

    #[test]
    fn un_bloqueador_opaco_apaga_la_luz_por_completo() {
        let scene = escena_con_bloqueador(Material::opaco(Color::white()));

        let t = super::transmitancia(
            &scene,
            &Vec3::new(0.0, -3.0, 0.0),
            &Vec3::new(0.0, 1.0, 0.0),
            6.0,
            0,
        );

        assert_eq!(t, Color::black());
    }

    #[test]
    fn un_bloqueador_detras_de_la_luz_no_tapa_nada() {
        // La luz esta a dos unidades y el cubo a tres: no se interpone.
        let scene = escena_con_bloqueador(Material::opaco(Color::white()));

        let t = super::transmitancia(
            &scene,
            &Vec3::new(0.0, -3.0, 0.0),
            &Vec3::new(0.0, 1.0, 0.0),
            2.0,
            0,
        );

        assert_eq!(t, Color::white());
    }

    #[test]
    fn la_sombra_del_teseracto_sale_azul() {
        // La sombra no es un parche gris: es lo que queda de la luz
        // despues de cruzar vidrio que se come el rojo.
        let scene = teseracto();
        let clave = scene
            .lights
            .iter()
            .find(|luz| luz.casts_shadows)
            .expect("la escena tiene una luz que proyecta sombra");

        let punto = Vec3::new(0.0, ALTURA_DEL_PISO, 0.0);
        let hacia_luz = clave.position - punto;
        let distancia = hacia_luz.magnitude();

        let t = super::transmitancia(&scene, &punto, &(hacia_luz / distancia), distancia, 0);

        assert!(t.b > t.r * 3.0, "la sombra deberia ser azul: {t:?}");
        assert!(t.b < 1.0, "algo tiene que estar tapando: {t:?}");
        assert!(t.r > 0.0, "el cascaron no es opaco: {t:?}");
    }

    #[test]
    fn una_luz_sin_sombras_atraviesa_lo_que_sea() {
        // La del teseracto nace dentro del nucleo opaco. Si proyectara
        // sombra, el piso quedaria negro justo debajo del objeto que se
        // supone que lo ilumina.
        let scene = teseracto();
        let interna = scene
            .lights
            .iter()
            .find(|luz| !luz.casts_shadows)
            .expect("la escena tiene la luz del propio teseracto");

        let bajo_el_objeto = punto_del_piso(0.0, 0.0);
        let solo_ambiente = super::difusa(
            &Scene {
                objects: vec![],
                lights: vec![],
                background: Color::black(),
                bloom: crate::bloom::Bloom::apagado(),
            },
            &bajo_el_objeto,
            albedo_del_piso(),
        );

        let con_la_interna = super::difusa(
            &Scene {
                objects: scene.objects,
                lights: vec![*interna],
                background: Color::black(),
                bloom: crate::bloom::Bloom::apagado(),
            },
            &bajo_el_objeto,
            albedo_del_piso(),
        );

        assert!(
            con_la_interna.b > solo_ambiente.b * 3.0,
            "{} contra {}",
            con_la_interna.b,
            solo_ambiente.b
        );
    }

    #[test]
    fn el_piso_recibe_el_charco_de_luz_del_teseracto() {
        // Dos puntos del piso, uno al lado del objeto y otro lejos. La
        // caida cuadratica de la luz interna tiene que separarlos.
        let scene = teseracto();

        let cerca = super::difusa(&scene, &punto_del_piso(1.6, 1.6), albedo_del_piso());
        let lejos = super::difusa(&scene, &punto_del_piso(25.0, 25.0), albedo_del_piso());

        assert!(cerca.b > lejos.b * 5.0, "{} contra {}", cerca.b, lejos.b);
    }

    #[test]
    fn el_charco_del_piso_es_mas_azul_que_su_propio_color() {
        // El piso es gris. Si lo que se ve en el charco fuera su albedo, el
        // azul y el rojo irian parejos.
        let scene = teseracto();
        let albedo = albedo_del_piso();

        let charco = super::difusa(&scene, &punto_del_piso(1.6, 1.6), albedo);

        assert!(
            charco.b / charco.r > albedo.b / albedo.r * 2.0,
            "charco {charco:?} contra albedo {albedo:?}"
        );
    }

    /// Impacto sobre la cara superior del piso en `(x, z)`, mirando hacia
    /// abajo. Sirve para sombrear un punto del piso sin tener que buscar el
    /// pixel que lo ve.
    fn punto_del_piso(x: f32, z: f32) -> Hit {
        let arriba = Vec3::new(x, ALTURA_DEL_PISO + 5.0, z);
        let ray = Ray::new(arriba, Vec3::new(0.0, -1.0, 0.0));

        Hit::new(
            &ray,
            5.0,
            Vec3::new(0.0, 1.0, 0.0),
            nalgebra_glm::Vec2::zeros(),
        )
    }

    fn albedo_del_piso() -> Color {
        teseracto().objects[2].material.albedo
    }

    /// Un solo cubo de lado 2 en el origen, con el material que se le pase.
    fn escena_con_bloqueador(material: Material) -> Scene {
        use crate::cuboid::Cuboid;
        use crate::scene::Object;

        Scene {
            objects: vec![Object {
                shape: Cuboid::cubo(Vec3::zeros(), 2.0),
                material,
            }],
            lights: vec![],
            background: Color::black(),
            bloom: crate::bloom::Bloom::apagado(),
        }
    }

    /// Un pixel que toca el cascaron pero no el nucleo, buscado sobre la
    /// fila central desde la orilla hacia adentro.
    fn borde_del_cascaron(scene: &Scene, camera: &Camera) -> (usize, usize) {
        let y = ALTO / 2;
        let nucleo = scene.objects[1].shape;

        for x in 0..ANCHO {
            let ray = camera.ray_from_pixel(x, y, ANCHO, ALTO);

            if scene.cast(&ray).is_some() && nucleo.thickness(&ray) == 0.0 {
                return (x, y);
            }
        }

        panic!("no hay pixel de cascaron sin nucleo detras");
    }
}
