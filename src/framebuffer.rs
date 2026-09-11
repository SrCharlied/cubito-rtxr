use crate::color::Color;
use std::error::Error;
use std::fs;
use std::io::Write;
use std::path::Path;

/// Lienzo doble: lo que el trazador escribe y lo que la pantalla recibe.
///
/// `hdr` guarda color **lineal y sin acotar**, que es donde el nucleo del
/// teseracto puede valer cuatro veces el blanco y el post-proceso todavia
/// puede hacer algo con esa informacion. `buffer` es su version empacada a
/// `0x00RRGGBB`, que es justo lo que `minifb` presenta sin conversion
/// intermedia.
///
/// Los dos existen porque el orden importa: el bloom tiene que leer el
/// rango completo **antes** de que el recorte lo tire. `pack` es el paso
/// que cruza de uno al otro, y despues de el las dos vistas coinciden.
pub struct Framebuffer {
    pub width: usize,
    pub height: usize,
    /// Vista empacada de `hdr`. Valida despues de `pack`.
    pub buffer: Vec<u32>,
    /// Lo que el trazador escribe: lineal, sin recortar.
    pub hdr: Vec<Color>,
}

impl Framebuffer {
    pub fn new(width: usize, height: usize) -> Self {
        Framebuffer {
            width,
            height,
            buffer: vec![0; width * height],
            hdr: vec![Color::black(); width * height],
        }
    }

    pub fn clear(&mut self, color: Color) {
        let empacado = color.to_u32();

        for pixel in self.hdr.iter_mut() {
            *pixel = color;
        }
        for pixel in self.buffer.iter_mut() {
            *pixel = empacado;
        }
    }

    /// Escribe un pixel en el buffer lineal.
    ///
    /// No toca `buffer`: hacerlo aqui empacaria un valor que el bloom
    /// todavia va a cambiar. Quien dibuja termina con `pack`.
    ///
    /// Fuera de rango no hace nada. El renderer nunca se sale, pero tampoco
    /// vale reventar por un redondeo de la interfaz.
    pub fn set(&mut self, x: usize, y: usize, color: Color) {
        if x < self.width && y < self.height {
            self.hdr[y * self.width + x] = color;
        }
    }

    /// Empaca `hdr` en `buffer`: codifica a sRGB y recorta.
    pub fn pack(&mut self) {
        for (pixel, color) in self.buffer.iter_mut().zip(&self.hdr) {
            *pixel = color.to_u32();
        }
    }

    /// Guarda la imagen como PPM binario (P6).
    ///
    /// PPM y no PNG para no arrastrar un codificador como dependencia: el
    /// formato son tres bytes por pixel detras de una cabecera de texto, y
    /// cualquier visor decente —GIMP, IrfanView, ffmpeg— lo abre. Sirve
    /// para dejar evidencia del render sin depender de una captura de
    /// pantalla.
    ///
    /// Escribe `buffer`, asi que hay que haber llamado `pack` antes.
    pub fn save_ppm(&self, path: &Path) -> Result<(), Box<dyn Error>> {
        if let Some(directorio) = path.parent() {
            if !directorio.as_os_str().is_empty() {
                fs::create_dir_all(directorio)?;
            }
        }

        let mut bytes = Vec::with_capacity(self.width * self.height * 3 + 32);
        write!(bytes, "P6\n{} {}\n255\n", self.width, self.height)?;

        for pixel in &self.buffer {
            bytes.push(((pixel >> 16) & 0xFF) as u8);
            bytes.push(((pixel >> 8) & 0xFF) as u8);
            bytes.push((pixel & 0xFF) as u8);
        }

        fs::write(path, bytes)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arranca_en_negro_con_el_tamano_pedido() {
        let fb = Framebuffer::new(4, 3);

        assert_eq!(fb.buffer.len(), 12);
        assert_eq!(fb.hdr.len(), 12);
        assert!(fb.buffer.iter().all(|&pixel| pixel == 0));
    }

    #[test]
    fn set_escribe_en_la_fila_correcta() {
        let mut fb = Framebuffer::new(4, 3);
        fb.set(1, 2, Color::white());
        fb.pack();

        assert_eq!(fb.buffer[2 * 4 + 1], 0xFFFFFF);
    }

    #[test]
    fn set_no_empaca_hasta_que_se_lo_pidan() {
        // Es lo que deja al bloom leer el rango completo: si `set`
        // empacara, el recorte ocurriria antes del post-proceso.
        let mut fb = Framebuffer::new(4, 3);
        fb.set(0, 0, Color::white());

        assert_eq!(fb.buffer[0], 0);
        assert_eq!(fb.hdr[0], Color::white());
    }

    #[test]
    fn el_rango_alto_sobrevive_en_hdr_y_se_recorta_al_empacar() {
        let mut fb = Framebuffer::new(4, 3);
        fb.set(0, 0, Color::new(4.0, 4.0, 4.0));

        assert_eq!(fb.hdr[0].r, 4.0);

        fb.pack();
        assert_eq!(fb.buffer[0], 0xFFFFFF);
    }

    #[test]
    fn set_fuera_de_rango_no_toca_nada() {
        let mut fb = Framebuffer::new(4, 3);
        fb.set(9, 9, Color::white());
        fb.pack();

        assert!(fb.buffer.iter().all(|&pixel| pixel == 0));
    }

    #[test]
    fn clear_pinta_las_dos_vistas() {
        let mut fb = Framebuffer::new(4, 3);
        let fondo = Color::from_hex(0x112233);
        fb.clear(fondo);

        assert!(fb.buffer.iter().all(|&pixel| pixel == 0x112233));
        assert!(fb.hdr.iter().all(|&pixel| pixel == fondo));
    }
}
