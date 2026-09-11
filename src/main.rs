//! Ciclo de ventana del cubito.
//!
//! Solo tres responsabilidades: abrir la ventana, leer el teclado y
//! presentar el framebuffer. Todo lo que se puede probar sin ventana vive
//! en la libreria del paquete.

use minifb::{Key, KeyRepeat, Window, WindowOptions};
use std::f32::consts::PI;
use std::thread::sleep;
use std::time::Duration;

use cubito_rtxr::framebuffer::Framebuffer;
use cubito_rtxr::renderer::{render, Shading};
use cubito_rtxr::scene::{camara_inicial, jaula, preset_inicial, teseracto, teseracto_texturizado};
use std::path::Path;

const WIDTH: usize = 800;
const HEIGHT: usize = 600;

/// Cuanto gira la camara por cuadro mientras se sostiene una flecha. Tres
/// grados: una vuelta completa toma dos segundos a 60 cuadros.
const ROTATION_SPEED: f32 = PI / 60.0;

/// Cuanto se acerca o aleja la camara por paso de zoom, como fraccion del
/// radio actual.
///
/// Relativo y no absoluto para que el zoom se sienta igual de rapido de
/// lejos que de cerca: un paso fijo en unidades de mundo seria
/// imperceptible desde lejos y brusco desde cerca.
const ZOOM_FRACTION: f32 = 0.06;

/// Cual de las escenas se esta mostrando.
///
/// Un enum y no un booleano desde que son tres: con dos banderas sueltas
/// existirian estados que no significan nada, como «texturizado y jaula a
/// la vez».
#[derive(Clone, Copy, PartialEq, Eq)]
enum Escenario {
    Teseracto,
    Texturizado,
    Jaula,
}

fn main() {
    let frame_delay = Duration::from_millis(16);

    let mut framebuffer = Framebuffer::new(WIDTH, HEIGHT);
    let mut window = Window::new("cubito-rtxr", WIDTH, HEIGHT, WindowOptions::default())
        .expect("no se pudo abrir la ventana");

    // Las dos escenas se construyen una sola vez y se alternan por
    // referencia: son baratas, pero rehacerlas en cada cambio de tecla
    // volveria a decodificar colores y a reservar vectores por nada.
    let escena_teseracto = teseracto();
    let escena_jaula = jaula();

    // La texturizada es la unica que carga archivos, asi que es la unica
    // que puede faltar. Un asset ausente no tumba el programa: se avisa una
    // vez, con el comando que los genera, y las otras dos escenas siguen
    // disponibles. Abortar por esto dejaria sin ver lo que si esta listo.
    let escena_texturizada = match teseracto_texturizado(Path::new("assets")) {
        Ok(scene) => Some(scene),
        Err(fallo) => {
            eprintln!("aviso: la escena texturizada no esta disponible");
            eprintln!("  {fallo}");
            eprintln!("  generala con: python tools/generar_texturas.py");
            None
        }
    };

    let mut escenario = Escenario::Teseracto;

    let mut camera = camara_inicial();
    let hero = preset_inicial();
    let mut shading = Shading::Diffuse;

    println!("cubito-rtxr");
    println!("  flechas  orbitar     W / S / rueda  zoom     R  encuadre inicial");
    println!("  T  teseracto     X  teseracto texturizado     J  jaula difusa");
    println!("  1  normales     2  albedo     3  difusa     Escape  salir");

    // El primer cuadro cuenta como cambio pendiente, para que la ventana
    // arranque con la imagen ya trazada.
    let mut redibujar = true;

    while window.is_open() && !window.is_key_down(Key::Escape) {
        // --------------------------------------------------------- orbita
        //
        // `is_key_down` y no `is_key_pressed`: orbitar es un gesto que se
        // sostiene, y con el evento de pulsacion habria que soltar y
        // volver a apretar por cada tres grados.
        let orbita = [
            (Key::Left, ROTATION_SPEED, 0.0),
            (Key::Right, -ROTATION_SPEED, 0.0),
            (Key::Up, 0.0, -ROTATION_SPEED),
            (Key::Down, 0.0, ROTATION_SPEED),
        ];

        for (tecla, delta_yaw, delta_pitch) in orbita {
            if window.is_key_down(tecla) {
                camera.orbit(delta_yaw, delta_pitch);
                redibujar = true;
            }
        }

        // ----------------------------------------------------------- zoom
        let paso = camera.radius() * ZOOM_FRACTION;

        if window.is_key_down(Key::W) {
            camera.zoom(-paso);
            redibujar = true;
        }
        if window.is_key_down(Key::S) {
            camera.zoom(paso);
            redibujar = true;
        }

        // La rueda entrega magnitudes muy distintas segun el sistema
        // operativo y el raton; solo se usa su signo.
        if let Some((_, vertical)) = window.get_scroll_wheel() {
            if vertical.abs() > f32::EPSILON {
                camera.zoom(-paso * vertical.signum());
                redibujar = true;
            }
        }

        // ------------------------------------------------- modo y reinicio
        //
        // Aqui si `is_key_pressed`: cambiar de modo o volver al encuadre
        // son eventos, y repetirlos cada cuadro no significa nada.
        let modos = [
            (Key::Key1, Shading::Normals),
            (Key::Key2, Shading::Albedo),
            (Key::Key3, Shading::Diffuse),
        ];

        for (tecla, modo) in modos {
            if window.is_key_pressed(tecla, KeyRepeat::No) && shading != modo {
                shading = modo;
                redibujar = true;
            }
        }

        if window.is_key_pressed(Key::R, KeyRepeat::No) {
            camera.restore(hero);
            redibujar = true;
        }

        // ---------------------------------------------------------- escena
        //
        // Las tres lecturas de la misma figura, a un toque de distancia:
        // alternar entre ellas es la forma mas rapida de ver que aporta
        // cada capa sobre la difusa desnuda.
        let escenas = [
            (Key::T, Escenario::Teseracto),
            (Key::X, Escenario::Texturizado),
            (Key::J, Escenario::Jaula),
        ];

        for (tecla, siguiente) in escenas {
            if !window.is_key_pressed(tecla, KeyRepeat::No) || escenario == siguiente {
                continue;
            }

            if siguiente == Escenario::Texturizado && escena_texturizada.is_none() {
                println!("faltan las texturas: python tools/generar_texturas.py");
                continue;
            }

            escenario = siguiente;
            redibujar = true;
        }

        // -------------------------------------------------------- presentar
        //
        // Se traza solo cuando algo cambio. Las dos escenas cuestan: el
        // teseracto atraviesa vidrio y pasa el halo seis veces sobre la
        // imagen, y la jaula resuelve catorce objetos y dos rayos de sombra
        // por impacto. No hay razon para pagarlo por cuadro para repetir la
        // misma imagen.
        if redibujar {
            let scene = match escenario {
                Escenario::Teseracto => &escena_teseracto,
                // El respaldo no se alcanza —la tecla no deja entrar aqui
                // sin texturas—, pero evita un panico si eso cambiara.
                Escenario::Texturizado => escena_texturizada.as_ref().unwrap_or(&escena_teseracto),
                Escenario::Jaula => &escena_jaula,
            };

            render(&mut framebuffer, scene, &camera, shading);
            redibujar = false;
        }

        // `update_with_buffer` va siempre, tambien cuando no se dibujo: es
        // lo que bombea los eventos de la ventana. Sin la llamada, el
        // sistema la da por colgada.
        window
            .update_with_buffer(&framebuffer.buffer, WIDTH, HEIGHT)
            .expect("no se pudo presentar el cuadro");

        sleep(frame_delay);
    }
}
