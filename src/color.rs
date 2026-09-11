use std::ops::{Add, Mul, Sub};

/// Decodifica un canal sRGB de 0..1 a intensidad lineal.
///
/// Un literal hexadecimal es un valor de paleta, escrito como se ve en un
/// selector de color; nunca energia. Sumar y multiplicar luz sobre valores
/// sRGB da resultados apagados y sucios, asi que todo lo que entra por
/// `from_hex` se lleva a lineal primero.
fn srgb_a_lineal(canal: f32) -> f32 {
    if canal <= 0.04045 {
        canal / 12.92
    } else {
        ((canal + 0.055) / 1.055).powf(2.4)
    }
}

/// Codifica intensidad lineal de vuelta a sRGB, que es lo que la pantalla
/// espera recibir. Es la operacion inversa de `srgb_a_lineal` y se aplica
/// una sola vez, al empacar el pixel.
fn lineal_a_srgb(canal: f32) -> f32 {
    if canal <= 0.003_130_8 {
        canal * 12.92
    } else {
        1.055 * canal.powf(1.0 / 2.4) - 0.055
    }
}

/// Color en espacio **lineal**, con canales sin acotar por arriba.
///
/// No se recorta a 1.0 en cada operacion a proposito: una suma de aportes
/// de luz puede pasarse de uno en un paso intermedio y volver a bajar
/// despues. El recorte ocurre una vez, en `to_u32`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

impl Color {
    pub fn new(r: f32, g: f32, b: f32) -> Self {
        Color { r, g, b }
    }

    pub fn black() -> Self {
        Color::new(0.0, 0.0, 0.0)
    }

    pub fn white() -> Self {
        Color::new(1.0, 1.0, 1.0)
    }

    /// Interpreta un `0xRRGGBB` **sRGB** y lo lleva a lineal.
    pub fn from_hex(hex: u32) -> Self {
        let canal = |desplazamiento: u32| ((hex >> desplazamiento) & 0xFF) as f32 / 255.0;

        Color::new(
            srgb_a_lineal(canal(16)),
            srgb_a_lineal(canal(8)),
            srgb_a_lineal(canal(0)),
        )
    }

    /// Empaca a `0x00RRGGBB` para el framebuffer, codificando a sRGB y
    /// recortando. Es el unico punto donde el color deja de ser energia y
    /// pasa a ser un pixel.
    pub fn to_u32(self) -> u32 {
        let byte = |canal: f32| (lineal_a_srgb(canal.clamp(0.0, 1.0)) * 255.0).round() as u32;

        (byte(self.r) << 16) | (byte(self.g) << 8) | byte(self.b)
    }
}

impl Add for Color {
    type Output = Color;

    fn add(self, otro: Color) -> Color {
        Color::new(self.r + otro.r, self.g + otro.g, self.b + otro.b)
    }
}

/// Escalar por intensidad: atenuacion, coseno de Lambert, fraccion de
/// ambiente.
impl Mul<f32> for Color {
    type Output = Color;

    fn mul(self, escalar: f32) -> Color {
        Color::new(self.r * escalar, self.g * escalar, self.b * escalar)
    }
}

/// Producto canal a canal: es como el albedo tine la luz que recibe.
impl Mul<Color> for Color {
    type Output = Color;

    fn mul(self, otro: Color) -> Color {
        Color::new(self.r * otro.r, self.g * otro.g, self.b * otro.b)
    }
}

/// Resta canal a canal. La usa el bloom en dos lugares: para quedarse con
/// lo que pasa del umbral y para leer una ventana de una suma acumulada.
/// Puede dar canales negativos, y esta bien: es aritmetica intermedia, y el
/// recorte ocurre una sola vez, al empacar.
impl Sub for Color {
    type Output = Color;

    fn sub(self, otro: Color) -> Color {
        Color::new(self.r - otro.r, self.g - otro.g, self.b - otro.b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_blanco_y_el_negro_sobreviven_el_viaje_de_ida_y_vuelta() {
        assert_eq!(Color::from_hex(0xFFFFFF).to_u32(), 0xFFFFFF);
        assert_eq!(Color::from_hex(0x000000).to_u32(), 0x000000);
    }

    #[test]
    fn un_color_cualquiera_sobrevive_el_viaje_de_ida_y_vuelta() {
        // Redondeo a 8 bits de por medio, asi que se admite un paso de
        // diferencia por canal.
        for hex in [0x336699, 0xC0FFEE, 0x7F7F7F] {
            let round_trip = Color::from_hex(hex).to_u32();

            for desplazamiento in [16, 8, 0] {
                let original = ((hex >> desplazamiento) & 0xFF) as i32;
                let obtenido = ((round_trip >> desplazamiento) & 0xFF) as i32;

                assert!(
                    (original - obtenido).abs() <= 1,
                    "{hex:06X} -> {round_trip:06X}"
                );
            }
        }
    }

    #[test]
    fn el_medio_gris_srgb_no_es_la_mitad_de_la_energia() {
        // La prueba de que la decodificacion ocurre: 0x808080 es el gris
        // que se ve a medio camino, pero en energia esta cerca de 0.216.
        let medio = Color::from_hex(0x808080);

        assert!(medio.r > 0.2 && medio.r < 0.23, "{}", medio.r);
    }

    #[test]
    fn to_u32_recorta_en_vez_de_desbordarse() {
        let sobreexpuesto = Color::new(4.0, 1.0, -2.0);

        assert_eq!(sobreexpuesto.to_u32(), 0xFFFF00);
    }

    #[test]
    fn la_resta_admite_canales_negativos() {
        let diferencia = Color::new(0.25, 0.5, 0.75) - Color::white();

        assert!(diferencia.r < 0.0 && diferencia.g < 0.0 && diferencia.b < 0.0);
    }

    #[test]
    fn el_producto_por_canal_tine_la_luz() {
        let rojo = Color::new(1.0, 0.0, 0.0);
        let blanca = Color::white();

        assert_eq!(rojo * blanca, rojo);
        assert_eq!(rojo * Color::new(0.0, 1.0, 0.0), Color::black());
    }
}
