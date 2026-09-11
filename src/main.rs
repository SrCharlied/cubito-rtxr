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
use cubito_rtxr::scene::{camara_inicial, cubito, preset_inicial, teseracto};

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

fn main() {
    let frame_delay = Duration::from_millis(16);

    let mut framebuffer = Framebuffer::new(WIDTH, HEIGHT);
    let mut window = Window::new("cubito-rtxr", WIDTH, HEIGHT, WindowOptions::default())
        .expect("no se pudo abrir la ventana");

    // Las dos escenas se construyen una sola vez y se alternan por
    // referencia: son baratas, pero rehacerlas en cada cambio de tecla
    // volveria a decodificar colores y a reservar vectores por nada.
    let escena_teseracto = teseracto();
    let escena_cubo = cubito();
    let mut es_teseracto = true;

    let mut camera = camara_inicial();
    let hero = preset_inicial();
    let mut shading = Shading::Diffuse;

    println!("cubito-rtxr");
    println!("  flechas  orbitar     W / S / rueda  zoom     R  encuadre inicial");
    println!("  T  teseracto     C  cubo mate");
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
        // El cubo mate se deja a un toque de distancia porque es la
        // referencia: alternar entre los dos es la forma mas rapida de ver
        // que aporta cada capa del teseracto.
        for (tecla, quiere_teseracto) in [(Key::T, true), (Key::C, false)] {
            if window.is_key_pressed(tecla, KeyRepeat::No) && es_teseracto != quiere_teseracto {
                es_teseracto = quiere_teseracto;
                redibujar = true;
            }
        }

        // -------------------------------------------------------- presentar
        //
        // Se traza solo cuando algo cambio. El teseracto cuesta mas que el
        // cubo mate —cada rayo primario atraviesa el vidrio y el halo pasa
        // seis veces sobre la imagen—, y no hay razon para pagarlo por
        // cuadro para repetir la misma imagen.
        if redibujar {
            let scene = if es_teseracto {
                &escena_teseracto
            } else {
                &escena_cubo
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
