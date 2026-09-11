//! Imagenes que modulan un material, y su lector de PPM.
//!
//! El formato es el mismo P6 que `Framebuffer` ya sabe escribir, por la
//! misma razon que alla: son tres bytes por pixel detras de una cabecera de
//! texto, asi que ni el generador ni el renderer necesitan un codificador
//! de PNG. Pesa mas en disco y no cuesta ninguna dependencia.

use crate::color::Color;
use nalgebra_glm::Vec2;
use std::error::Error;
use std::fs;
use std::path::Path;

/// Una imagen muestreable, en color **lineal**.
///
/// La conversion ocurre al cargar, una sola vez, y no en cada muestreo: un
/// byte de un archivo de imagen es sRGB por convencion, igual que un
/// literal hexadecimal, y mezclar luz con valores sRGB da resultados
/// apagados. Ver `color`.
pub struct Texture {
    pub width: usize,
    pub height: usize,
    pixels: Vec<Color>,
}

impl Texture {
    /// Lee un PPM binario (P6) de 8 bits por canal.
    pub fn load_ppm(path: &Path) -> Result<Texture, Box<dyn Error>> {
        let bytes =
            fs::read(path).map_err(|e| format!("no se pudo leer {}: {e}", path.display()))?;

        let (width, height, inicio) = cabecera(&bytes, path)?;
        let esperados = width * height * 3;

        if bytes.len() - inicio < esperados {
            return Err(format!(
                "{}: la cabecera anuncia {width} x {height} pero solo hay {} bytes de pixel",
                path.display(),
                bytes.len() - inicio
            )
            .into());
        }

        let pixels = bytes[inicio..inicio + esperados]
            .chunks_exact(3)
            .map(|rgb| {
                Color::from_hex(((rgb[0] as u32) << 16) | ((rgb[1] as u32) << 8) | rgb[2] as u32)
            })
            .collect();

        Ok(Texture {
            width,
            height,
            pixels,
        })
    }

    /// Color en la coordenada `uv`, con interpolacion bilineal.
    ///
    /// Bilineal y no vecino mas cercano porque estas texturas se ven de
    /// cerca: una veta de cristal muestreada a saltos se convierte en una
    /// escalera de bloques, y el cubo entero pasa a leerse como pixel art.
    ///
    /// Las coordenadas se **envuelven**: la columna que sigue a la ultima es
    /// otra vez la primera. Es lo que aprovecha que el generador haga
    /// texturas periodicas, y lo que evita que la interpolacion del ultimo
    /// texel se apoye en un borde inventado.
    ///
    /// La `v` se invierte porque la fila 0 de una imagen es la de arriba,
    /// mientras que `v = 0` es el borde inferior de la cara.
    pub fn sample(&self, uv: &Vec2) -> Color {
        if self.pixels.is_empty() {
            return Color::white();
        }

        // El medio texel de corrimiento pone la muestra en el **centro** del
        // texel, que es donde su valor es exacto; sin el, la interpolacion
        // sale desplazada media celda.
        let x = uv.x * self.width as f32 - 0.5;
        let y = (1.0 - uv.y) * self.height as f32 - 0.5;

        let x0 = x.floor();
        let y0 = y.floor();
        let tx = x - x0;
        let ty = y - y0;
        let (x0, y0) = (x0 as i32, y0 as i32);

        let arriba = mezclar(self.texel(x0, y0), self.texel(x0 + 1, y0), tx);
        let abajo = mezclar(self.texel(x0, y0 + 1), self.texel(x0 + 1, y0 + 1), tx);

        mezclar(arriba, abajo, ty)
    }

    fn texel(&self, x: i32, y: i32) -> Color {
        let x = x.rem_euclid(self.width as i32) as usize;
        let y = y.rem_euclid(self.height as i32) as usize;

        self.pixels[y * self.width + x]
    }
}

fn mezclar(a: Color, b: Color, t: f32) -> Color {
    a * (1.0 - t) + b * t
}

/// Lee la cabecera P6 y devuelve el tamano y donde empiezan los pixeles.
///
/// La cabecera son tokens separados por espacios en blanco, con comentarios
/// que arrancan en `#` y llegan hasta el fin de linea. Despues del ultimo
/// token va **exactamente un** byte de espacio, y de ahi en adelante todo
/// es dato binario: por eso no se puede analizar con un `split` sobre el
/// archivo entero, que se comeria bytes de pixel que casualmente sean
/// espacios.
fn cabecera(bytes: &[u8], path: &Path) -> Result<(usize, usize, usize), Box<dyn Error>> {
    let mut cursor = 0;
    let mut tokens: Vec<String> = Vec::with_capacity(4);

    while tokens.len() < 4 {
        // Saltar espacios y comentarios.
        loop {
            match bytes.get(cursor) {
                Some(b) if b.is_ascii_whitespace() => cursor += 1,
                Some(b'#') => {
                    while !matches!(bytes.get(cursor), None | Some(b'\n')) {
                        cursor += 1;
                    }
                }
                _ => break,
            }
        }

        let inicio = cursor;
        while matches!(bytes.get(cursor), Some(b) if !b.is_ascii_whitespace() && *b != b'#') {
            cursor += 1;
        }

        if inicio == cursor {
            return Err(format!("{}: cabecera PPM incompleta", path.display()).into());
        }

        tokens.push(String::from_utf8_lossy(&bytes[inicio..cursor]).into_owned());
    }

    if tokens[0] != "P6" {
        return Err(format!(
            "{}: solo se admite PPM binario (P6), no {}",
            path.display(),
            tokens[0]
        )
        .into());
    }

    let width: usize = tokens[1].parse()?;
    let height: usize = tokens[2].parse()?;
    let maximo: u32 = tokens[3].parse()?;

    if maximo != 255 {
        return Err(format!(
            "{}: solo se admiten 8 bits por canal, no un maximo de {maximo}",
            path.display()
        )
        .into());
    }

    if width == 0 || height == 0 {
        return Err(format!("{}: textura de lado cero", path.display()).into());
    }

    // Un unico byte de espacio separa la cabecera del dato binario.
    Ok((width, height, cursor + 1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Escribe un PPM de prueba en el directorio temporal y devuelve su
    /// ruta. El nombre lleva el caso para que dos pruebas en paralelo no se
    /// pisen el archivo.
    fn ppm_temporal(nombre: &str, contenido: &[u8]) -> std::path::PathBuf {
        let ruta = std::env::temp_dir().join(format!("cubito_rtxr_{nombre}.ppm"));
        let mut archivo = fs::File::create(&ruta).expect("crear el temporal");
        archivo.write_all(contenido).expect("escribir el temporal");

        ruta
    }

    /// Dos por dos: negro, blanco / rojo puro, negro.
    fn cuatro_texels() -> Vec<u8> {
        let mut bytes = b"P6\n2 2\n255\n".to_vec();
        bytes.extend_from_slice(&[0, 0, 0, 255, 255, 255, 255, 0, 0, 0, 0, 0]);

        bytes
    }

    #[test]
    fn lee_el_tamano_y_los_pixeles() {
        let ruta = ppm_temporal("tamano", &cuatro_texels());
        let textura = Texture::load_ppm(&ruta).expect("deberia cargar");

        assert_eq!((textura.width, textura.height), (2, 2));
        assert_eq!(textura.pixels.len(), 4);
    }

    #[test]
    fn admite_comentarios_en_la_cabecera() {
        let mut bytes = b"P6\n# generado por tools/generar_texturas.py\n2 2\n255\n".to_vec();
        bytes.extend_from_slice(&[0, 0, 0, 255, 255, 255, 255, 0, 0, 0, 0, 0]);

        let ruta = ppm_temporal("comentario", &bytes);
        let textura = Texture::load_ppm(&ruta).expect("deberia cargar");

        assert_eq!((textura.width, textura.height), (2, 2));
    }

    #[test]
    fn el_muestreo_en_el_centro_de_un_texel_lo_devuelve_intacto() {
        // Fila 0 de la imagen es la de arriba, que corresponde a v = 0.75
        // en una textura de dos texels de alto.
        let ruta = ppm_temporal("centro", &cuatro_texels());
        let textura = Texture::load_ppm(&ruta).expect("deberia cargar");

        let blanco = textura.sample(&Vec2::new(0.75, 0.75));

        assert!((blanco.r - 1.0).abs() < 1e-5, "{blanco:?}");
        assert!((blanco.g - 1.0).abs() < 1e-5, "{blanco:?}");
    }

    #[test]
    fn el_muestreo_interpola_entre_texels() {
        // Justo entre el negro y el blanco de la fila de arriba: tiene que
        // salir un valor intermedio, no uno de los dos.
        let ruta = ppm_temporal("interpola", &cuatro_texels());
        let textura = Texture::load_ppm(&ruta).expect("deberia cargar");

        let medio = textura.sample(&Vec2::new(0.5, 0.75));

        assert!(medio.r > 0.01 && medio.r < 0.99, "{medio:?}");
    }

    #[test]
    fn el_muestreo_envuelve_en_los_bordes() {
        // Un uv fuera de rango no puede reventar ni devolver basura: la
        // textura es periodica, asi que 1.25 es lo mismo que 0.25.
        let ruta = ppm_temporal("envuelve", &cuatro_texels());
        let textura = Texture::load_ppm(&ruta).expect("deberia cargar");

        let dentro = textura.sample(&Vec2::new(0.25, 0.25));
        let fuera = textura.sample(&Vec2::new(1.25, 0.25));

        assert!((dentro.r - fuera.r).abs() < 1e-5, "{dentro:?} vs {fuera:?}");
    }

    #[test]
    fn los_bytes_se_decodifican_de_srgb_a_lineal() {
        // Un 128 en el archivo es medio gris **a la vista**, que en energia
        // esta cerca de 0.216. Si saliera 0.5, la decodificacion no ocurrio.
        let mut bytes = b"P6\n1 1\n255\n".to_vec();
        bytes.extend_from_slice(&[128, 128, 128]);

        let ruta = ppm_temporal("srgb", &bytes);
        let textura = Texture::load_ppm(&ruta).expect("deberia cargar");
        let gris = textura.sample(&Vec2::new(0.5, 0.5));

        assert!(gris.r > 0.2 && gris.r < 0.23, "{gris:?}");
    }

    #[test]
    fn rechaza_un_formato_que_no_es_p6() {
        let ruta = ppm_temporal("p3", b"P3\n2 2\n255\n0 0 0\n");

        assert!(Texture::load_ppm(&ruta).is_err());
    }

    #[test]
    fn rechaza_un_archivo_truncado() {
        // La cabecera promete cuatro pixeles y solo hay uno.
        let mut bytes = b"P6\n2 2\n255\n".to_vec();
        bytes.extend_from_slice(&[255, 255, 255]);

        let ruta = ppm_temporal("truncado", &bytes);

        assert!(Texture::load_ppm(&ruta).is_err());
    }

    #[test]
    fn rechaza_una_ruta_que_no_existe() {
        let ruta = std::env::temp_dir().join("cubito_rtxr_no_existe_jamas.ppm");

        assert!(Texture::load_ppm(&ruta).is_err());
    }
}
