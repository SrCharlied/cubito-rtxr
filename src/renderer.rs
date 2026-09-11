use crate::camera::Camera;
use crate::color::Color;
use crate::framebuffer::Framebuffer;
use crate::hit::Hit;
use crate::light::{direct_diffuse, AMBIENT};
use crate::ray::Ray;
use crate::scene::Scene;

/// Que se pinta en cada pixel.
///
/// Los tres modos son los tres escalones de construccion del proyecto, y se
/// dejan disponibles porque siguen siendo las herramientas de diagnostico:
/// cuando la imagen sombreada se ve mal, `Normals` dice si el problema esta
/// en la geometria o en la luz.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shading {
    /// Raycaster puro: la normal de la cara, llevada a color. Sin luces de
    /// por medio, cada cara del cubo sale de un color distinto y fijo.
    Normals,
    /// Solo el color propio del objeto. Silueta correcta, cero volumen.
    Albedo,
    /// Ambiente mas difusa. Es el modo de la entrega.
    Diffuse,
}

/// Traza un rayo primario y devuelve el color que le corresponde.
///
/// Un solo rebote: el rayo toca o no toca. Sin reflexion, sin refraccion y
/// sin rayos de sombra —el cubo es convexo y es el unico objeto, asi que no
/// puede darse sombra a si mismo ni recibirla de nadie—.
pub fn trace(scene: &Scene, ray: &Ray, shading: Shading) -> Color {
    let Some((hit, objeto)) = scene.cast(ray) else {
        return scene.background;
    };

    match shading {
        Shading::Normals => color_por_normal(&hit),
        Shading::Albedo => objeto.albedo,
        Shading::Diffuse => difusa(scene, &hit, objeto.albedo),
    }
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

        color = color
            + direct_diffuse(
                albedo,
                &hit.normal,
                &(hacia_luz / distancia),
                light.color,
                atenuacion,
            );
    }

    color
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

/// Dibuja la escena completa sobre el framebuffer.
///
/// Un rayo por pixel, sin antialiasing: las aristas del cubo salen duras.
/// Es lo que corresponde a este paso; suavizarlas es muestrear varias veces
/// por pixel y eso pertenece a otro.
pub fn render(framebuffer: &mut Framebuffer, scene: &Scene, camera: &Camera, shading: Shading) {
    let (width, height) = (framebuffer.width, framebuffer.height);

    for y in 0..height {
        for x in 0..width {
            let ray = camera.ray_from_pixel(x, y, width, height);

            framebuffer.set(x, y, trace(scene, &ray, shading));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{camara_inicial, cubito};
    use nalgebra_glm::Vec3;

    /// Resolucion chica: estas pruebas verifican relaciones entre pixeles,
    /// no la imagen final, y a 80 x 60 corren en un parpadeo.
    const ANCHO: usize = 80;
    const ALTO: usize = 60;

    fn render_de_prueba(shading: Shading) -> Framebuffer {
        let mut framebuffer = Framebuffer::new(ANCHO, ALTO);
        render(&mut framebuffer, &cubito(), &camara_inicial(), shading);

        framebuffer
    }

    #[test]
    fn el_fondo_queda_donde_no_hay_cubo() {
        let framebuffer = render_de_prueba(Shading::Diffuse);
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
        let framebuffer = render_de_prueba(Shading::Diffuse);
        let centro = framebuffer.buffer[(ALTO / 2) * ANCHO + ANCHO / 2];

        assert_ne!(centro, cubito().background.to_u32());
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

                let color = super::difusa(&scene, &hit, objeto.albedo);
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

                let color = super::difusa(&scene, &hit, objeto.albedo);
                let piso = objeto.albedo.r * AMBIENT;

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
        let framebuffer = render_de_prueba(Shading::Albedo);
        let scene = cubito();

        let fondo = scene.background.to_u32();
        let albedo = scene.objects[0].albedo.to_u32();

        assert!(framebuffer
            .buffer
            .iter()
            .all(|&pixel| pixel == fondo || pixel == albedo));
    }
}
