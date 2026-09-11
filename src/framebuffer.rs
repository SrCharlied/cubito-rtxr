use crate::color::Color;
use std::error::Error;
use std::fs;
use std::io::Write;
use std::path::Path;

/// Lienzo en memoria: pixeles `0x00RRGGBB`, que es justo lo que `minifb`
/// espera presentar sin conversion intermedia.
pub struct Framebuffer {
    pub width: usize,
    pub height: usize,
    pub buffer: Vec<u32>,
}

impl Framebuffer {
    pub fn new(width: usize, height: usize) -> Self {
        Framebuffer {
            width,
            height,
            buffer: vec![0; width * height],
        }
    }

    pub fn clear(&mut self, color: Color) {
        let empacado = color.to_u32();

        for pixel in self.buffer.iter_mut() {
            *pixel = empacado;
        }
    }

    /// Escribe un pixel. Fuera de rango no hace nada: el renderer nunca se
    /// sale, pero tampoco vale reventar por un redondeo de la interfaz.
    pub fn set(&mut self, x: usize, y: usize, color: Color) {
        if x < self.width && y < self.height {
            self.buffer[y * self.width + x] = color.to_u32();
        }
    }

    /// Guarda la imagen como PPM binario (P6).
    ///
    /// PPM y no PNG para no arrastrar un codificador como dependencia: el
    /// formato son tres bytes por pixel detras de una cabecera de texto, y
    /// cualquier visor decente —GIMP, IrfanView, ffmpeg— lo abre. Sirve
    /// para dejar evidencia del render sin depender de una captura de
    /// pantalla.
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
        assert!(fb.buffer.iter().all(|&pixel| pixel == 0));
    }

    #[test]
    fn set_escribe_en_la_fila_correcta() {
        let mut fb = Framebuffer::new(4, 3);
        fb.set(1, 2, Color::white());

        assert_eq!(fb.buffer[2 * 4 + 1], 0xFFFFFF);
    }

    #[test]
    fn set_fuera_de_rango_no_toca_nada() {
        let mut fb = Framebuffer::new(4, 3);
        fb.set(9, 9, Color::white());

        assert!(fb.buffer.iter().all(|&pixel| pixel == 0));
    }

    #[test]
    fn clear_pinta_todo_el_lienzo() {
        let mut fb = Framebuffer::new(4, 3);
        fb.clear(Color::from_hex(0x112233));

        assert!(fb.buffer.iter().all(|&pixel| pixel == 0x112233));
    }
}
