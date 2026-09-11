//! Raytracer minimo: un cubo, una luz difusa y una camara orbital.
//!
//! Todo lo que se puede probar sin abrir una ventana vive aqui —rayos,
//! geometria, camara, color y el trazado en si—. El binario se queda solo
//! con lo que no se puede probar de esa forma: crear la ventana, leer el
//! teclado y presentar el framebuffer.
//!
//! El orden de construccion fue el del enunciado: primero el raycaster
//! —camara, rayo primario, framebuffer— y despues la forma.

pub mod aabb;
pub mod camera;
pub mod color;
pub mod cuboid;
pub mod framebuffer;
pub mod hit;
pub mod light;
pub mod ray;
pub mod ray_intersect;
pub mod renderer;
pub mod scene;

/// Margen para despegar un rayo de la superficie que lo origino y para
/// recortar impactos degenerados justo sobre el origen.
///
/// Un solo valor canonico para todo el proyecto, y no tres epsilons
/// distintos regados por los modulos que haya que reconciliar despues.
pub const EPSILON: f32 = 1e-4;
