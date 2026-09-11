//! Halo alrededor de lo que brilla mas que la pantalla.
//!
//! Es post-proceso, no trazado: opera sobre la imagen ya calculada. Y es lo
//! que hace que el nucleo del teseracto se **lea** como encendido. Sin
//! halo, un pixel a cuatro veces el blanco y otro justo en el blanco se
//! empacan los dos como `0xFFFFFF` y se ven identicos; el derrame sobre los
//! vecinos es la unica pista que queda del rango que se perdio al recortar.
//!
//! Por eso trabaja sobre el buffer **lineal y sin acotar**, antes de
//! empacar. Aplicado despues del recorte no tendria de donde sacar la
//! informacion.

use crate::color::Color;

/// Ajustes del halo.
#[derive(Debug, Clone, Copy)]
pub struct Bloom {
    /// A partir de que brillo un pixel derrama. Por encima de 1.0 solo
    /// derrama lo que la pantalla ya no puede mostrar.
    pub threshold: f32,
    /// Cuanto del halo se suma de vuelta.
    pub intensity: f32,
    /// Radio de cada pasada, en pixeles.
    pub radius: usize,
    /// Cuantas pasadas de caja. Tres aproximan bien una gaussiana, que es
    /// lo que evita que el halo se vea cuadrado.
    pub passes: usize,
}

impl Bloom {
    /// Sin halo. Es lo que corresponde a una escena sin nada sobreexpuesto:
    /// no hay rango perdido que insinuar.
    pub const fn apagado() -> Self {
        Bloom {
            threshold: 1.0,
            intensity: 0.0,
            radius: 0,
            passes: 0,
        }
    }

    /// El halo del teseracto: umbral apenas debajo del blanco para que el
    /// marco de las aristas tambien derrame, y radio ancho para que el
    /// resplandor se despegue de la silueta.
    pub const fn teseracto() -> Self {
        Bloom {
            threshold: 0.9,
            intensity: 0.6,
            radius: 16,
            passes: 3,
        }
    }

    pub fn esta_activo(&self) -> bool {
        self.intensity > 0.0 && self.radius > 0 && self.passes > 0
    }
}

/// Suma el halo sobre la imagen, en sitio.
///
/// `pixels` va en espacio lineal y sin acotar, en orden de filas.
pub fn apply(pixels: &mut [Color], width: usize, height: usize, cfg: &Bloom) {
    if !cfg.esta_activo() || width == 0 || height == 0 || pixels.len() != width * height {
        return;
    }

    // Lo que pasa del umbral, y solo eso. Un pixel a 0.5 con umbral 0.9 no
    // aporta nada al halo; uno a 3.0 aporta 2.1.
    let mut halo: Vec<Color> = pixels.iter().map(|&c| exceso(c, cfg.threshold)).collect();
    let mut temporal = vec![Color::black(); halo.len()];
    let mut acumulado = Vec::with_capacity(width.max(height) + 1);

    for _ in 0..cfg.passes {
        desenfocar_en_x(
            &halo,
            &mut temporal,
            width,
            height,
            cfg.radius,
            &mut acumulado,
        );
        desenfocar_en_y(
            &temporal,
            &mut halo,
            width,
            height,
            cfg.radius,
            &mut acumulado,
        );
    }

    for (pixel, aporte) in pixels.iter_mut().zip(halo) {
        *pixel = *pixel + aporte * cfg.intensity;
    }
}

/// Cuanto pasa del umbral, por canal, sin bajar de cero.
fn exceso(color: Color, umbral: f32) -> Color {
    Color::new(
        (color.r - umbral).max(0.0),
        (color.g - umbral).max(0.0),
        (color.b - umbral).max(0.0),
    )
}

/// Desenfoque de caja horizontal.
///
/// Con sumas acumuladas: cada pixel de salida es una resta entre dos
/// entradas de la tabla, asi que el costo no depende del radio. La
/// alternativa —sumar `2r+1` vecinos por pixel— con radio 16 y tres pasadas
/// serian cien millones de operaciones por cuadro, y el halo dejaria de ser
/// gratis.
///
/// La tabla acumulada pierde algo de precision en las ultimas columnas, que
/// para un desenfoque no tiene ninguna consecuencia visible.
fn desenfocar_en_x(
    origen: &[Color],
    destino: &mut [Color],
    width: usize,
    height: usize,
    radius: usize,
    acumulado: &mut Vec<Color>,
) {
    for y in 0..height {
        let fila = y * width;

        acumulado.clear();
        acumulado.push(Color::black());
        let mut suma = Color::black();

        for x in 0..width {
            suma = suma + origen[fila + x];
            acumulado.push(suma);
        }

        for x in 0..width {
            // La ventana se recorta contra el borde de la imagen y se
            // promedia por lo que quedo dentro. Extenderla con ceros
            // oscureceria el halo justo en los bordes.
            let desde = x.saturating_sub(radius);
            let hasta = (x + radius + 1).min(width);

            destino[fila + x] =
                (acumulado[hasta] - acumulado[desde]) * (1.0 / (hasta - desde) as f32);
        }
    }
}

/// Desenfoque de caja vertical. Identico al horizontal salvo por el paso
/// entre muestras, que aqui es una fila entera.
fn desenfocar_en_y(
    origen: &[Color],
    destino: &mut [Color],
    width: usize,
    height: usize,
    radius: usize,
    acumulado: &mut Vec<Color>,
) {
    for x in 0..width {
        acumulado.clear();
        acumulado.push(Color::black());
        let mut suma = Color::black();

        for y in 0..height {
            suma = suma + origen[y * width + x];
            acumulado.push(suma);
        }

        for y in 0..height {
            let desde = y.saturating_sub(radius);
            let hasta = (y + radius + 1).min(height);

            destino[y * width + x] =
                (acumulado[hasta] - acumulado[desde]) * (1.0 / (hasta - desde) as f32);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ANCHO: usize = 33;
    const ALTO: usize = 33;
    const CENTRO: usize = (ALTO / 2) * ANCHO + ANCHO / 2;

    fn imagen_con_un_punto_brillante(brillo: f32) -> Vec<Color> {
        let mut pixels = vec![Color::black(); ANCHO * ALTO];
        pixels[CENTRO] = Color::new(brillo, brillo, brillo);

        pixels
    }

    fn suave() -> Bloom {
        Bloom {
            threshold: 1.0,
            intensity: 1.0,
            radius: 3,
            passes: 2,
        }
    }

    #[test]
    fn apagado_no_toca_la_imagen() {
        let original = imagen_con_un_punto_brillante(8.0);
        let mut pixels = original.clone();

        apply(&mut pixels, ANCHO, ALTO, &Bloom::apagado());

        assert_eq!(pixels, original);
    }

    #[test]
    fn un_pixel_sobreexpuesto_derrama_sobre_sus_vecinos() {
        let mut pixels = imagen_con_un_punto_brillante(8.0);

        apply(&mut pixels, ANCHO, ALTO, &suave());

        assert!(pixels[CENTRO + 1].r > 0.0, "el vecino inmediato");
        assert!(pixels[CENTRO + ANCHO].r > 0.0, "el vecino de abajo");
    }

    #[test]
    fn el_derrame_decae_con_la_distancia() {
        let mut pixels = imagen_con_un_punto_brillante(8.0);

        apply(&mut pixels, ANCHO, ALTO, &suave());

        let cerca = pixels[CENTRO + 1].r;
        let lejos = pixels[CENTRO + 4].r;

        assert!(cerca > lejos, "{cerca} contra {lejos}");
    }

    #[test]
    fn lo_que_no_pasa_el_umbral_no_derrama() {
        let mut pixels = imagen_con_un_punto_brillante(0.75);
        let original = pixels.clone();

        apply(&mut pixels, ANCHO, ALTO, &suave());

        assert_eq!(pixels, original);
    }

    #[test]
    fn el_halo_solo_agrega_luz() {
        // Nunca debe restar: un halo que oscurece algun pixel seria un
        // signo cambiado en la resta de la tabla acumulada.
        let original = imagen_con_un_punto_brillante(8.0);
        let mut pixels = original.clone();

        apply(&mut pixels, ANCHO, ALTO, &suave());

        for (despues, antes) in pixels.iter().zip(&original) {
            assert!(
                despues.r >= antes.r - 1e-5,
                "{} contra {}",
                despues.r,
                antes.r
            );
        }
    }

    #[test]
    fn el_halo_se_reparte_sin_inventar_energia() {
        // Las dos pasadas de caja reparten el exceso; la suma del halo no
        // puede pasar del exceso original por la intensidad.
        let mut pixels = imagen_con_un_punto_brillante(8.0);
        let cfg = suave();
        let exceso_original = 8.0 - cfg.threshold;

        apply(&mut pixels, ANCHO, ALTO, &cfg);

        let total: f32 = pixels.iter().map(|c| c.r).sum();
        let esperado = 8.0 + exceso_original * cfg.intensity;

        assert!(total <= esperado + 1e-3, "{total} contra {esperado}");
    }

    #[test]
    fn una_imagen_del_tamano_equivocado_se_ignora() {
        // No es una ruta alcanzable desde el renderer, pero tampoco vale
        // indexar fuera de rango si alguna vez lo fuera.
        let mut pixels = vec![Color::white(); 10];

        apply(&mut pixels, ANCHO, ALTO, &suave());

        assert_eq!(pixels, vec![Color::white(); 10]);
    }
}
