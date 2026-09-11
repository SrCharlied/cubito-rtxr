use crate::ray::Ray;
use nalgebra_glm::{normalize, Vec3};
use std::f32::consts::PI;

/// Limite del pitch: un poco antes de los polos. Justo en el polo la
/// direccion de vista queda paralela a `up`, el producto cruz da el vector
/// cero y la base se vuelve degenerada: la imagen se rompe.
const PITCH_LIMIT: f32 = PI / 2.0 - 0.1;

/// Campo de vision vertical por omision: 60 grados.
pub const DEFAULT_VERTICAL_FOV: f32 = PI / 3.0;

/// Encuadre guardado, para poder volver a una vista sin reconstruir la
/// camara. No incluye los limites de radio ni el campo de vision: esos son
/// propiedades de la escena y del proyecto, no del encuadre, y restaurar
/// una vista no debe cambiarlos.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraPreset {
    pub eye: Vec3,
    pub center: Vec3,
}

/// Camara orbital: el ojo gira sobre una esfera alrededor de `center` y
/// siempre mira hacia el.
///
/// El estado se guarda en cartesianas y los angulos se derivan al orbitar,
/// en vez de guardar yaw/pitch/radio y reconstruir el ojo. Asi hay una sola
/// fuente de verdad —donde esta el ojo— y no dos representaciones que
/// puedan desincronizarse.
#[derive(Debug, Clone, Copy)]
pub struct Camera {
    pub eye: Vec3,
    /// Eje de giro y punto de encuadre a la vez. Con un solo cubo
    /// centrado, separarlos no compraria nada.
    pub center: Vec3,
    pub up: Vec3,
    pub vertical_fov: f32,
    pub min_radius: f32,
    pub max_radius: f32,
    /// Altura por debajo de la cual el ojo no puede bajar.
    ///
    /// Existe por el piso. Sin este tope, orbitar hacia abajo mete la
    /// camara bajo la losa y lo unico que se ve es su cara inferior, que
    /// es una pantalla negra: el usuario no lee eso como «estoy debajo del
    /// piso», lo lee como que el programa se rompio.
    ///
    /// `f32::NEG_INFINITY` lo desactiva, y es el valor por omision: una
    /// escena sin piso no tiene por que perder media esfera de orbita.
    pub min_eye_height: f32,
}

impl Camera {
    /// Los limites del zoom arrancan derivados del radio inicial: acercarse
    /// a un tercio y alejarse al triple es un rango comodo para inspeccionar
    /// una sola pieza.
    const MIN_RADIUS_FACTOR: f32 = 0.35;
    const MAX_RADIUS_FACTOR: f32 = 3.0;

    pub fn new(eye: Vec3, center: Vec3, up: Vec3, vertical_fov: f32) -> Self {
        let radius = (eye - center).magnitude();

        Camera {
            eye,
            center,
            up,
            vertical_fov,
            min_radius: radius * Self::MIN_RADIUS_FACTOR,
            max_radius: radius * Self::MAX_RADIUS_FACTOR,
            min_eye_height: f32::NEG_INFINITY,
        }
    }

    /// Fija los limites del zoom explicitamente.
    pub fn with_radius_limits(mut self, min_radius: f32, max_radius: f32) -> Self {
        self.min_radius = min_radius;
        self.max_radius = max_radius;

        self
    }

    /// Impide que el ojo baje de `altura`. Ver `min_eye_height`.
    ///
    /// Se aplica de inmediato: una camara construida ya por debajo del piso
    /// sube al ras en vez de esperar al primer movimiento.
    pub fn with_min_eye_height(mut self, altura: f32) -> Self {
        self.min_eye_height = altura;
        self.respetar_el_piso();

        self
    }

    /// Distancia del ojo al centro. Es la magnitud que el zoom modifica y
    /// que la orbita debe conservar.
    pub fn radius(&self) -> f32 {
        (self.eye - self.center).magnitude()
    }

    /// Direccion normalizada del ojo hacia el centro.
    pub fn forward(&self) -> Vec3 {
        (self.center - self.eye).normalize()
    }

    /// Lleva un vector de coordenadas de camara a coordenadas del mundo.
    ///
    /// Los rayos se generan siempre igual —hacia -Z, con la pantalla en el
    /// plano XY—, asi que nacen en el sistema de la camara. Este cambio de
    /// base los reexpresa en el del mundo, que es donde vive el cubo. Es
    /// **lo unico** que hace que orbitar cambie la imagen: la geometria no
    /// se mueve, se mueve la base.
    pub fn basis_change(&self, vector: &Vec3) -> Vec3 {
        let forward = self.forward();
        let right = forward.cross(&self.up).normalize();

        // El `up` recibido es una intencion, no necesariamente
        // perpendicular a la vista. Recalcularlo con el producto cruz de
        // los otros dos garantiza que los tres ejes sean ortogonales.
        let up = right.cross(&forward).normalize();

        // La camara ve hacia -Z, de ahi el signo del ultimo termino.
        let rotado = vector.x * right + vector.y * up - vector.z * forward;

        rotado.normalize()
    }

    /// Gira el ojo alrededor de `center` conservando la distancia.
    ///
    /// Se pasa a esfericas, se suman los deltas y se vuelve a cartesianas.
    /// El pitch se recorta contra `PITCH_LIMIT`; el yaw no, porque dar la
    /// vuelta completa es legitimo.
    pub fn orbit(&mut self, delta_yaw: f32, delta_pitch: f32) {
        let radio_vector = self.eye - self.center;
        let radius = radio_vector.magnitude();

        // Yaw: angulo alrededor del eje Y. Pitch: altura sobre el plano XZ,
        // medido hacia abajo, asi que un delta **positivo baja el ojo**. Es
        // por eso que la flecha arriba manda un delta negativo: subir la
        // camara y bajar el pitch son lo mismo.
        let yaw_actual = radio_vector.z.atan2(radio_vector.x);
        let radio_xz = (radio_vector.x * radio_vector.x + radio_vector.z * radio_vector.z).sqrt();
        let pitch_actual = (-radio_vector.y).atan2(radio_xz);

        let yaw = (yaw_actual + delta_yaw) % (2.0 * PI);
        let pitch = (pitch_actual + delta_pitch).clamp(-PITCH_LIMIT, PITCH_LIMIT);

        self.eye = self.center
            + Vec3::new(
                radius * yaw.cos() * pitch.cos(),
                -radius * pitch.sin(),
                radius * yaw.sin() * pitch.cos(),
            );

        self.respetar_el_piso();
    }

    /// Sube el ojo al ras del piso si quedo por debajo, conservando el
    /// radio y el yaw.
    ///
    /// Se aplica **despues** de mover, y no como un recorte del angulo
    /// antes de mover, porque el pitch maximo admisible depende del radio:
    /// con el mismo angulo, un ojo lejano cae mas abajo que uno cercano. Un
    /// tope de angulo calculado una vez quedaria flojo tras un zoom y
    /// apretado tras el contrario. Corregir la posicion resultante vale
    /// para los dos gestos y para cualquier orden entre ellos.
    fn respetar_el_piso(&mut self) {
        if self.eye.y >= self.min_eye_height {
            return;
        }

        let radio_vector = self.eye - self.center;
        let radius = radio_vector.magnitude();

        if radius < f32::EPSILON {
            return;
        }

        let yaw = radio_vector.z.atan2(radio_vector.x);
        // `eye.y = center.y - radius * sin(pitch)`, de donde sale el pitch
        // que deja el ojo exactamente a la altura minima. El recorte cubre
        // el caso de un piso mas alto que el alcance del radio.
        let altura = ((self.center.y - self.min_eye_height) / radius).clamp(-1.0, 1.0);
        let pitch = altura.asin().min(PITCH_LIMIT);

        self.eye = self.center
            + Vec3::new(
                radius * yaw.cos() * pitch.cos(),
                -radius * pitch.sin(),
                radius * yaw.sin() * pitch.cos(),
            );
    }

    /// Acerca o aleja el ojo modificando el radio orbital.
    ///
    /// `delta` es un cambio de distancia con signo: negativo acerca. La
    /// direccion no se toca —el ojo se desliza sobre el mismo rayo que sale
    /// del centro—, asi que hacer zoom nunca reencuadra.
    ///
    /// El resultado se recorta a `min_radius..=max_radius`: sin ese recorte,
    /// acercarse de mas mete la camara dentro del cubo y alejarse de mas lo
    /// reduce a un punto.
    pub fn zoom(&mut self, delta: f32) {
        let radio_vector = self.eye - self.center;
        let radius = radio_vector.magnitude();

        // Con el ojo exactamente sobre el centro no hay direccion que
        // conservar. No es alcanzable orbitando, pero tampoco hay que
        // dividir entre cero por el.
        if radius < f32::EPSILON {
            return;
        }

        let nuevo = (radius + delta).clamp(self.min_radius, self.max_radius);

        self.eye = self.center + radio_vector * (nuevo / radius);

        // Alejarse con el ojo por debajo del centro tambien lo hunde: el
        // zoom escala la componente vertical junto con el resto.
        self.respetar_el_piso();
    }

    /// El encuadre actual, para poder volver a el mas tarde.
    pub fn preset(&self) -> CameraPreset {
        CameraPreset {
            eye: self.eye,
            center: self.center,
        }
    }

    /// Restaura un encuadre guardado, dejando intactos los limites de radio
    /// y el campo de vision.
    pub fn restore(&mut self, preset: CameraPreset) {
        self.eye = preset.eye;
        self.center = preset.center;
        self.respetar_el_piso();
    }

    /// Rayo primario que atraviesa el centro del pixel `(x, y)`.
    ///
    /// El `+ 0.5` muestrea el centro del pixel y no su borde: un pixel es
    /// un area y el renderer la representa por su punto medio.
    pub fn ray_from_pixel(&self, x: usize, y: usize, width: usize, height: usize) -> Ray {
        let screen_x = (2.0 * (x as f32 + 0.5)) / width as f32 - 1.0;
        let screen_y = -(2.0 * (y as f32 + 0.5)) / height as f32 + 1.0;

        self.ray_from_screen(screen_x, screen_y, width, height)
    }

    /// La proyeccion en perspectiva, unica en el proyecto.
    ///
    /// `screen_x` y `screen_y` van de -1 a 1, con la `y` ya invertida
    /// respecto del orden de filas de la imagen: la fila 0 es la de arriba,
    /// pero en el plano de proyeccion arriba es +1.
    ///
    /// # Precondicion
    ///
    /// `width` y `height` mayores que cero. Con cualquiera en cero el
    /// aspect ratio sale no finito y la direccion tambien: el rayo no
    /// falla, simplemente no impacta nada nunca.
    fn ray_from_screen(&self, screen_x: f32, screen_y: f32, width: usize, height: usize) -> Ray {
        let aspect_ratio = width as f32 / height as f32;

        // Media altura del plano de proyeccion, que esta a una unidad de la
        // camara. Abrir el campo de vision ensancha el plano.
        let escala = (self.vertical_fov / 2.0).tan();

        let direccion = normalize(&Vec3::new(
            screen_x * aspect_ratio * escala,
            screen_y * escala,
            -1.0,
        ));

        Ray::new(self.eye, self.basis_change(&direccion))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nalgebra_glm::dot;

    fn camara() -> Camera {
        Camera::new(
            Vec3::new(0.0, 0.0, 5.0),
            Vec3::zeros(),
            Vec3::new(0.0, 1.0, 0.0),
            DEFAULT_VERTICAL_FOV,
        )
    }

    #[test]
    fn la_orbita_conserva_el_radio() {
        let mut camera = camara();
        let inicial = camera.radius();

        for _ in 0..12 {
            camera.orbit(PI / 7.0, PI / 11.0);

            assert!(
                (camera.radius() - inicial).abs() < 1e-4,
                "radio {} contra {inicial}",
                camera.radius()
            );
        }
    }

    #[test]
    fn la_orbita_nunca_cruza_el_polo() {
        let mut camera = camara();

        // Muchas vueltas hacia arriba: el recorte debe frenar el ojo antes
        // de que la base se vuelva degenerada.
        for _ in 0..40 {
            camera.orbit(0.0, PI / 8.0);
        }

        let forward = camera.forward();
        let paralelismo = dot(&forward, &camera.up).abs();

        assert!(paralelismo < 0.999, "paralelismo {paralelismo}");
    }

    #[test]
    fn media_vuelta_de_yaw_deja_el_ojo_del_otro_lado() {
        let mut camera = camara();
        camera.orbit(PI, 0.0);

        assert!((camera.eye.z + 5.0).abs() < 1e-4, "{:?}", camera.eye);
    }

    #[test]
    fn el_zoom_respeta_los_limites() {
        let mut camera = camara().with_radius_limits(2.0, 8.0);

        camera.zoom(-100.0);
        assert!((camera.radius() - 2.0).abs() < 1e-4, "{}", camera.radius());

        camera.zoom(100.0);
        assert!((camera.radius() - 8.0).abs() < 1e-4, "{}", camera.radius());
    }

    #[test]
    fn el_zoom_no_reencuadra() {
        let mut camera = camara();
        let direccion = camera.forward();

        camera.zoom(-1.5);

        let delta = (camera.forward() - direccion).magnitude();
        assert!(delta < 1e-5, "la vista se movio {delta}");
    }

    #[test]
    fn el_rayo_central_apunta_al_centro() {
        let camera = camara();
        // Resolucion par: no hay pixel central exacto, asi que se toman los
        // dos del medio y el promedio de sus direcciones debe apuntar al
        // centro.
        let izquierda = camera.ray_from_pixel(399, 299, 800, 600).direction;
        let derecha = camera.ray_from_pixel(400, 300, 800, 600).direction;
        let promedio = ((izquierda + derecha) * 0.5).normalize();

        let desvio = (promedio - camera.forward()).magnitude();
        assert!(desvio < 1e-3, "desvio {desvio}");
    }

    #[test]
    fn todos_los_rayos_nacen_en_el_ojo() {
        let camera = camara();

        for (x, y) in [(0, 0), (799, 0), (0, 599), (799, 599), (400, 300)] {
            let ray = camera.ray_from_pixel(x, y, 800, 600);

            assert_eq!(ray.origin, camera.eye);
            assert!((ray.direction.magnitude() - 1.0).abs() < 1e-5);
        }
    }

    #[test]
    fn la_fila_cero_apunta_hacia_arriba() {
        // Con la camara mirando hacia -Z y `up` en +Y, el primer renglon
        // de la imagen tiene que salir por encima del rayo central.
        let camera = camara();

        let arriba = camera.ray_from_pixel(400, 0, 800, 600).direction;
        let abajo = camera.ray_from_pixel(400, 599, 800, 600).direction;

        assert!(arriba.y > 0.0, "{arriba:?}");
        assert!(abajo.y < 0.0, "{abajo:?}");
    }

    #[test]
    fn sin_piso_la_orbita_puede_bajar_hasta_el_limite_del_pitch() {
        // El tope es opcional: una escena sin piso no pierde media esfera.
        // Un delta de pitch **positivo** baja el ojo: ver `orbit`.
        let mut camera = camara();

        for _ in 0..40 {
            camera.orbit(0.0, PI / 8.0);
        }

        assert!(camera.eye.y < -4.0, "{:?}", camera.eye);
    }

    #[test]
    fn con_piso_la_orbita_se_frena_al_ras() {
        let mut camera = camara().with_min_eye_height(-1.0);

        for _ in 0..40 {
            camera.orbit(PI / 13.0, PI / 8.0);

            assert!(camera.eye.y >= -1.0 - 1e-4, "{:?}", camera.eye);
        }
    }

    #[test]
    fn el_frenazo_conserva_el_radio() {
        // Subir el ojo al ras no puede acercarlo ni alejarlo: seria un zoom
        // fantasma al llegar al piso.
        let mut camera = camara().with_min_eye_height(-1.0);
        let inicial = camera.radius();

        for _ in 0..20 {
            camera.orbit(0.3, 0.4);
        }

        assert!(
            (camera.radius() - inicial).abs() < 1e-4,
            "{}",
            camera.radius()
        );
    }

    #[test]
    fn alejarse_tampoco_hunde_el_ojo_bajo_el_piso() {
        // El zoom escala la componente vertical junto con el resto, asi que
        // con el ojo por debajo del centro, alejarse lo hunde.
        let mut camera = camara().with_min_eye_height(-1.0);
        camera.orbit(0.0, 0.3);
        assert!(camera.eye.y < 0.0, "el ojo deberia haber bajado del centro");

        camera.zoom(100.0);

        assert!(camera.eye.y >= -1.0 - 1e-4, "{:?}", camera.eye);
    }

    #[test]
    fn una_camara_construida_bajo_el_piso_sube_al_ras() {
        let camera = Camera::new(
            Vec3::new(0.0, -5.0, 1.0),
            Vec3::zeros(),
            Vec3::new(0.0, 1.0, 0.0),
            DEFAULT_VERTICAL_FOV,
        )
        .with_min_eye_height(-1.0);

        assert!(camera.eye.y >= -1.0 - 1e-4, "{:?}", camera.eye);
    }

    #[test]
    fn restore_devuelve_el_encuadre_sin_tocar_los_limites() {
        let mut camera = camara().with_radius_limits(2.0, 8.0);
        let hero = camera.preset();

        camera.orbit(1.0, 0.5);
        camera.zoom(1.0);
        camera.restore(hero);

        assert_eq!(camera.preset(), hero);
        assert_eq!((camera.min_radius, camera.max_radius), (2.0, 8.0));
    }
}
