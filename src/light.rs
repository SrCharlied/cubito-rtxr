use crate::color::Color;
use nalgebra_glm::{dot, Vec3};

/// Fraccion de ambiente. Deliberadamente baja: no es fisica, es el piso que
/// impide que la cara no iluminada quede en negro absoluto y el cubo pierda
/// su silueta contra el fondo.
pub const AMBIENT: f32 = 0.08;

/// Luz puntual con caida cuadratica.
///
/// `reference_distance` es lo que hace manejable la intensidad: a esa
/// distancia la atenuacion vale exactamente `intensity`, asi que se puede
/// colocar la luz donde convenga sin volver a calibrar el brillo a ojo.
#[derive(Debug, Clone, Copy)]
pub struct Light {
    pub position: Vec3,
    pub color: Color,
    pub intensity: f32,
    pub reference_distance: f32,
    /// Si un objeto entre esta luz y el punto puede bloquearla.
    ///
    /// No todas las luces representan una lampara. La del teseracto es un
    /// resumen de la luz que escapa de **todo** su volumen: nace en el
    /// centro, que esta dentro del nucleo opaco, asi que un rayo de sombra
    /// hacia ella daria siempre bloqueado y el piso quedaria a oscuras bajo
    /// el objeto que se supone que lo ilumina. Apagarle las sombras es
    /// reconocer que el punto es una simplificacion de una fuente
    /// extendida, no una lampara escondida dentro de una caja.
    pub casts_shadows: bool,
}

impl Light {
    /// Luz puntual completa. Las demas constructoras se apoyan en esta.
    pub fn puntual(position: Vec3, color: Color, intensity: f32, reference_distance: f32) -> Self {
        Light {
            position,
            color,
            intensity,
            reference_distance: reference_distance.max(1e-3),
            casts_shadows: true,
        }
    }

    /// Luz blanca en `position`, con la distancia de referencia puesta en
    /// su propia distancia al origen: es el caso comun —la escena esta
    /// centrada en el origen— y deja la intensidad directamente legible.
    ///
    /// Una luz **en** el origen no tiene esa referencia natural, asi que
    /// cae en 1.0 y conviene fijarla con `con_distancia_de_referencia`.
    pub fn blanca(position: Vec3, intensity: f32) -> Self {
        let referencia = position.magnitude();
        let referencia = if referencia > 1e-3 { referencia } else { 1.0 };

        Light::puntual(position, Color::white(), intensity, referencia)
    }

    pub fn con_color(mut self, color: Color) -> Self {
        self.color = color;

        self
    }

    pub fn con_distancia_de_referencia(mut self, reference_distance: f32) -> Self {
        self.reference_distance = reference_distance.max(1e-3);

        self
    }

    /// Deja que esta luz atraviese cualquier cosa. Ver `casts_shadows`.
    pub fn sin_sombras(mut self) -> Self {
        self.casts_shadows = false;

        self
    }

    /// Cuanta de esta luz llega a `distance`.
    ///
    /// Cuadratica inversa normalizada: `atenuacion(reference_distance)` es
    /// `intensity`. El piso en la distancia evita que un punto pegado a la
    /// luz haga explotar el valor.
    pub fn attenuation(&self, distance: f32) -> f32 {
        let relativa = (distance / self.reference_distance).max(1e-3);

        self.intensity / (relativa * relativa)
    }
}

/// Coseno entre la normal y la direccion hacia la luz, recortado en cero.
///
/// Es toda la ley de Lambert: una superficie inclinada recibe la misma luz
/// repartida en mas area, y el coseno es esa fraccion. El recorte descarta
/// las luces que quedan **detras** de la superficie, que si no aportarian
/// un coseno negativo y oscurecerian el punto.
///
/// Espera los dos vectores normalizados.
pub fn lambert(normal: &Vec3, to_light: &Vec3) -> f32 {
    dot(normal, to_light).max(0.0)
}

/// Aporte difuso de **una** luz sobre un punto, ya atenuado.
///
/// No sabe de sombras ni de que objeto se trata: el renderer decide si esta
/// luz llega antes de llamar y le pasa la atenuacion ya resuelta, que es el
/// mismo numero que necesitaria un termino especular si mas adelante se
/// agrega. Devolver solo el aporte mantiene la funcion pura y comprobable
/// sin escena.
pub fn direct_diffuse(
    albedo: Color,
    normal: &Vec3,
    to_light: &Vec3,
    light_color: Color,
    attenuation: f32,
) -> Color {
    let difusa = lambert(normal, to_light);

    if difusa <= 0.0 {
        return Color::black();
    }

    albedo * (light_color * attenuation) * difusa
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn de_frente_el_coseno_vale_uno() {
        let normal = Vec3::new(0.0, 1.0, 0.0);

        assert!((lambert(&normal, &normal) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn a_cuarenta_y_cinco_grados_vale_la_raiz_de_dos_partido_dos() {
        let normal = Vec3::new(0.0, 1.0, 0.0);
        let hacia_luz = Vec3::new(1.0, 1.0, 0.0).normalize();

        let esperado = 2.0_f32.sqrt() / 2.0;
        assert!((lambert(&normal, &hacia_luz) - esperado).abs() < 1e-6);
    }

    #[test]
    fn una_luz_detras_de_la_superficie_no_aporta() {
        let normal = Vec3::new(0.0, 1.0, 0.0);

        assert_eq!(lambert(&normal, &Vec3::new(0.0, -1.0, 0.0)), 0.0);
        assert_eq!(lambert(&normal, &Vec3::new(1.0, 0.0, 0.0)), 0.0);
    }

    #[test]
    fn en_la_distancia_de_referencia_la_atenuacion_es_la_intensidad() {
        let luz = Light::blanca(Vec3::new(0.0, 0.0, 4.0), 1.4);

        assert!((luz.attenuation(4.0) - 1.4).abs() < 1e-5);
    }

    #[test]
    fn al_doble_de_distancia_llega_la_cuarta_parte() {
        let luz = Light::blanca(Vec3::new(0.0, 0.0, 4.0), 1.0);

        assert!((luz.attenuation(8.0) - 0.25).abs() < 1e-5);
    }

    #[test]
    fn una_luz_arranca_proyectando_sombra() {
        assert!(Light::blanca(Vec3::new(0.0, 4.0, 0.0), 1.0).casts_shadows);
        assert!(
            !Light::blanca(Vec3::new(0.0, 4.0, 0.0), 1.0)
                .sin_sombras()
                .casts_shadows
        );
    }

    #[test]
    fn una_luz_en_el_origen_no_explota() {
        // Sin el piso en la referencia, `position.magnitude()` seria cero y
        // la atenuacion, infinita.
        let luz = Light::blanca(Vec3::zeros(), 1.0);

        assert!(luz.attenuation(2.0).is_finite());
        assert!(luz.reference_distance > 0.0);
    }

    #[test]
    fn el_aporte_difuso_se_tine_con_el_albedo() {
        let rojo = Color::new(1.0, 0.0, 0.0);
        let normal = Vec3::new(0.0, 1.0, 0.0);

        let aporte = direct_diffuse(rojo, &normal, &normal, Color::white(), 1.0);

        assert_eq!(aporte, rojo);
    }

    #[test]
    fn una_luz_a_espaldas_de_la_cara_no_aporta_nada() {
        let normal = Vec3::new(0.0, 1.0, 0.0);
        let desde_abajo = Vec3::new(0.0, -1.0, 0.0);

        let aporte = direct_diffuse(Color::white(), &normal, &desde_abajo, Color::white(), 1.0);

        assert_eq!(aporte, Color::black());
    }
}
