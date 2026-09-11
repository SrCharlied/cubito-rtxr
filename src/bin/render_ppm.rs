//! Render sin ventana: traza un cuadro y lo escribe como PPM.
//!
//! Existe por dos razones. La primera es dejar evidencia reproducible del
//! render sin depender de una captura de pantalla. La segunda es poder
//! verificar el trazador en una maquina o una sesion sin servidor grafico,
//! donde `minifb` no puede abrir nada.
//!
//! ```text
//! cargo run --release --bin render_ppm -- salida.ppm --escena teseracto --yaw 45 --pitch 20
//! ```

use std::path::PathBuf;
use std::process::ExitCode;

use cubito_rtxr::framebuffer::Framebuffer;
use cubito_rtxr::renderer::{render, Shading};
use cubito_rtxr::scene::{camara_inicial, cubito, jaula, teseracto, Scene};

const WIDTH: usize = 800;
const HEIGHT: usize = 600;

/// Opciones de la linea de comandos, ya validadas.
struct Opciones {
    salida: PathBuf,
    yaw: f32,
    pitch: f32,
    shading: Shading,
    escena: fn() -> Scene,
}

fn main() -> ExitCode {
    let opciones = match parse(std::env::args().skip(1)) {
        Ok(opciones) => opciones,
        Err(fallo) => {
            eprintln!("error: {fallo}");
            eprintln!("uso: render_ppm [salida.ppm] [--escena teseracto|jaula|cubo]");
            eprintln!("                [--yaw grados] [--pitch grados]");
            eprintln!("                [--modo normales|albedo|difusa]");
            return ExitCode::FAILURE;
        }
    };

    let scene = (opciones.escena)();
    let mut camera = camara_inicial();
    camera.orbit(opciones.yaw.to_radians(), opciones.pitch.to_radians());

    let mut framebuffer = Framebuffer::new(WIDTH, HEIGHT);
    render(&mut framebuffer, &scene, &camera, opciones.shading);

    if let Err(fallo) = framebuffer.save_ppm(&opciones.salida) {
        eprintln!(
            "error: no se pudo escribir {}: {fallo}",
            opciones.salida.display()
        );
        return ExitCode::FAILURE;
    }

    println!(
        "{} x {} escrito en {}",
        WIDTH,
        HEIGHT,
        opciones.salida.display()
    );

    ExitCode::SUCCESS
}

/// Analiza los argumentos. Devuelve el motivo del rechazo en vez de abortar
/// para que `main` imprima el uso una sola vez.
fn parse(args: impl Iterator<Item = String>) -> Result<Opciones, String> {
    let mut opciones = Opciones {
        salida: PathBuf::from("cubito.ppm"),
        yaw: 0.0,
        pitch: 0.0,
        shading: Shading::Diffuse,
        escena: teseracto,
    };
    let mut args = args.peekable();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--yaw" => opciones.yaw = grados(args.next(), "--yaw")?,
            "--pitch" => opciones.pitch = grados(args.next(), "--pitch")?,
            "--escena" => {
                opciones.escena = match args.next().as_deref() {
                    Some("teseracto") => teseracto,
                    Some("jaula") => jaula,
                    Some("cubo") => cubito,
                    Some(otro) => return Err(format!("escena desconocida: {otro}")),
                    None => return Err("--escena espera un nombre".to_string()),
                }
            }
            "--modo" => {
                opciones.shading = match args.next().as_deref() {
                    Some("normales") => Shading::Normals,
                    Some("albedo") => Shading::Albedo,
                    Some("difusa") => Shading::Diffuse,
                    Some(otro) => return Err(format!("modo desconocido: {otro}")),
                    None => return Err("--modo espera un nombre".to_string()),
                }
            }
            otro if otro.starts_with("--") => return Err(format!("opcion desconocida: {otro}")),
            ruta => opciones.salida = PathBuf::from(ruta),
        }
    }

    Ok(opciones)
}

/// Lee un angulo en grados y lo rechaza si no es finito: un NaN aqui
/// produciria una camara sin direccion y una imagen de puro fondo, sin que
/// nada avisara.
fn grados(valor: Option<String>, bandera: &str) -> Result<f32, String> {
    let texto = valor.ok_or_else(|| format!("{bandera} espera un numero de grados"))?;
    let angulo: f32 = texto
        .parse()
        .map_err(|_| format!("{bandera}: {texto} no es un numero"))?;

    if !angulo.is_finite() || angulo.abs() > 360.0 * 4.0 {
        return Err(format!("{bandera}: {texto} fuera de rango"));
    }

    // El pitch lo recorta la camara; el yaw da la vuelta solo.
    Ok(angulo % 360.0)
}
