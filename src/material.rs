use crate::color::Color;
use nalgebra_glm::Vec2;

/// Como responde una superficie a la luz, y que luz pone de su cuenta.
///
/// Se construye con un encadenamiento: `Material::opaco(...)` da lo minimo
/// —solo albedo— y cada `con_*` agrega una propiedad. Asi el cubo simple se
/// escribe en una linea y el cascaron del teseracto en cuatro, sin que
/// ninguno tenga que nombrar campos que no le importan.
#[derive(Debug, Clone, Copy)]
pub struct Material {
    /// Color propio bajo luz difusa.
    pub albedo: Color,
    /// Luz que la superficie emite por si misma, en las mismas unidades
    /// que la que llega de una luz. No depende de ninguna lampara: es lo
    /// que hace que el nucleo se vea encendido aunque se apague todo.
    pub emission: Color,
    /// Fraccion del color que viene de **atras** de la superficie. 0 es
    /// opaco, 1 deja pasar todo.
    pub transmission: f32,
    /// Cuanta luz absorbe el medio por unidad de longitud, por canal
    /// (Beer-Lambert). Es lo que tine el vidrio: los canales que mas
    /// absorbe son los que faltan al salir.
    pub absorption: Color,
    /// Luz que el **medio** emite por unidad de longitud, no la superficie.
    ///
    /// Es lo que separa un cubo de vidrio de un cubo lleno de luz. La
    /// emision de superficie ilumina una lamina; esta se acumula con lo que
    /// el rayo recorre por dentro, asi que el volumen brilla mas por donde
    /// hay mas volumen, y el objeto deja de leerse como una jaula de
    /// alambre para leerse como una masa encendida.
    pub inner_glow: Color,
    /// Luz propia del marco de cada cara.
    pub edge_color: Color,
    /// Ancho del marco como fraccion de la cara. Cero lo apaga.
    pub edge_width: f32,
    /// Textura que modula el albedo, por indice dentro de `Scene::textures`.
    ///
    /// Un indice y no la textura misma: `Material` es `Copy` y viaja por
    /// valor en el camino caliente, mientras que una textura son cientos de
    /// kilobytes. Las texturas viven una sola vez en la escena y los
    /// materiales las nombran.
    pub albedo_map: Option<usize>,
    /// Textura que modula la emision. Es la que hace visible el dibujo en
    /// una superficie casi transparente: el albedo de un cascaron que
    /// transmite el noventa por ciento apenas se ve, pero la emision se
    /// suma sin pesar.
    pub emission_map: Option<usize>,
}

impl Material {
    /// Superficie mate: solo albedo. Todo lo demas apagado.
    pub fn opaco(albedo: Color) -> Self {
        Material {
            albedo,
            emission: Color::black(),
            transmission: 0.0,
            absorption: Color::black(),
            inner_glow: Color::black(),
            edge_color: Color::black(),
            edge_width: 0.0,
            albedo_map: None,
            emission_map: None,
        }
    }

    pub fn con_emision(mut self, emission: Color) -> Self {
        self.emission = emission;

        self
    }

    /// Deja pasar la luz de atras, tenida por `absorption`.
    ///
    /// La transmision se recorta a 0..1: un valor mayor que uno devolveria
    /// mas luz de la que entra y la imagen se iria aclarando sola con cada
    /// cascaron que se atraviese.
    pub fn traslucido(mut self, transmission: f32, absorption: Color) -> Self {
        self.transmission = transmission.clamp(0.0, 1.0);
        self.absorption = absorption;

        self
    }

    /// Luz repartida en el volumen, por unidad de longitud. Solo tiene
    /// efecto sobre un material que ademas transmita: sin transmision, el
    /// rayo nunca entra al medio.
    pub fn con_resplandor(mut self, inner_glow: Color) -> Self {
        self.inner_glow = inner_glow;

        self
    }

    pub fn con_marco(mut self, edge_color: Color, edge_width: f32) -> Self {
        self.edge_color = edge_color;
        self.edge_width = edge_width.clamp(0.0, 0.5);

        self
    }

    /// Modula el albedo con una textura de la escena.
    pub fn con_textura_de_albedo(mut self, indice: usize) -> Self {
        self.albedo_map = Some(indice);

        self
    }

    /// Modula la emision con una textura de la escena.
    pub fn con_textura_de_emision(mut self, indice: usize) -> Self {
        self.emission_map = Some(indice);

        self
    }

    /// Cuanto de marco hay en este punto de la cara: 1 justo sobre la
    /// arista, 0 a partir de `edge_width` hacia adentro.
    ///
    /// Sale del `uv` que la primitiva ya calculaba y que hasta ahora no
    /// usaba nadie: la distancia al borde mas cercano de la cara es
    /// `min(u, 1-u, v, 1-v)`.
    ///
    /// La transicion es un smoothstep y no una rampa lineal porque el
    /// cambio de pendiente en el limite del marco se ve como un escalon en
    /// el degradado, sobre todo con el bloom encima.
    pub fn edge_factor(&self, uv: &Vec2) -> f32 {
        if self.edge_width <= 0.0 {
            return 0.0;
        }

        let borde = uv.x.min(1.0 - uv.x).min(uv.y).min(1.0 - uv.y);

        if borde >= self.edge_width {
            return 0.0;
        }

        let t = 1.0 - borde / self.edge_width;

        t * t * (3.0 - 2.0 * t)
    }

    /// La luz que aporta el marco en este punto.
    pub fn edge_emission(&self, uv: &Vec2) -> Color {
        self.edge_color * self.edge_factor(uv)
    }
}

/// Ley de Beer-Lambert: que fraccion de la luz sobrevive a `distancia`
/// dentro de un medio que absorbe `absorption` por unidad de longitud.
///
/// Es lo que le da **volumen** al vidrio en vez de dejarlo como una lamina
/// tenida: el rayo que cruza el cubo por el centro recorre mas medio que el
/// que pasa raspando una esquina, y sale mas oscuro y mas saturado.
pub fn beer(absorption: Color, distancia: f32) -> Color {
    if distancia <= 0.0 {
        return Color::white();
    }

    Color::new(
        (-absorption.r * distancia).exp(),
        (-absorption.g * distancia).exp(),
        (-absorption.b * distancia).exp(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn con_marco() -> Material {
        Material::opaco(Color::black()).con_marco(Color::white(), 0.1)
    }

    #[test]
    fn el_centro_de_la_cara_no_tiene_marco() {
        assert_eq!(con_marco().edge_factor(&Vec2::new(0.5, 0.5)), 0.0);
    }

    #[test]
    fn la_arista_es_puro_marco() {
        assert!((con_marco().edge_factor(&Vec2::new(0.0, 0.5)) - 1.0).abs() < 1e-6);
        assert!((con_marco().edge_factor(&Vec2::new(0.5, 1.0)) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn el_marco_se_desvanece_hacia_adentro() {
        let material = con_marco();

        let cerca = material.edge_factor(&Vec2::new(0.02, 0.5));
        let lejos = material.edge_factor(&Vec2::new(0.08, 0.5));

        assert!(cerca > lejos, "{cerca} contra {lejos}");
        assert!(lejos > 0.0);
    }

    #[test]
    fn sin_ancho_no_hay_marco_en_ninguna_parte() {
        let material = Material::opaco(Color::white());

        for uv in [Vec2::zeros(), Vec2::new(0.5, 0.5), Vec2::new(1.0, 1.0)] {
            assert_eq!(material.edge_factor(&uv), 0.0);
        }
    }

    #[test]
    fn los_mapas_arrancan_vacios() {
        let material = Material::opaco(Color::white());

        assert_eq!(material.albedo_map, None);
        assert_eq!(material.emission_map, None);
    }

    #[test]
    fn el_resplandor_arranca_apagado() {
        assert_eq!(Material::opaco(Color::white()).inner_glow, Color::black());
    }

    #[test]
    fn la_transmision_se_recorta_al_rango_valido() {
        let material = Material::opaco(Color::white()).traslucido(1.8, Color::black());

        assert_eq!(material.transmission, 1.0);
    }

    #[test]
    fn a_distancia_cero_beer_no_absorbe() {
        assert_eq!(beer(Color::new(5.0, 5.0, 5.0), 0.0), Color::white());
    }

    #[test]
    fn beer_absorbe_mas_mientras_mas_medio_se_cruza() {
        let absorcion = Color::new(1.0, 1.0, 1.0);

        let corto = beer(absorcion, 0.5).r;
        let largo = beer(absorcion, 2.0).r;

        assert!(largo < corto, "{largo} contra {corto}");
        assert!(largo > 0.0, "la absorcion nunca llega a cero exacto");
    }

    #[test]
    fn beer_tine_el_medio() {
        // Absorbe rojo y deja pasar azul: lo que sale es azul.
        let filtro = beer(Color::new(3.0, 1.0, 0.1), 1.0);

        assert!(filtro.b > filtro.g && filtro.g > filtro.r, "{filtro:?}");
    }
}
