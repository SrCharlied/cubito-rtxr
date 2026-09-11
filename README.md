# cubito-rtxr

Raytracer minimo en Rust: **un cubo**, iluminacion **difusa** y **camara
orbital**. Sin sombras, sin reflexion, sin refraccion y sin texturas.

## Como correrlo

```bash
cargo run --release              # ventana interactiva
cargo test                       # 55 pruebas, sin ventana
```

Render sin ventana, util para dejar evidencia o para verificar en una
maquina sin servidor grafico:

```bash
cargo run --release --bin render_ppm -- salida.ppm --yaw 30 --pitch 15 --modo difusa
```

El PPM (P6) lo abre cualquier visor decente y no obliga a arrastrar un
codificador de PNG como dependencia.

## Controles

| Tecla | Efecto |
| --- | --- |
| Flechas | Orbitar alrededor del cubo |
| `W` / `S` / rueda | Acercar y alejar |
| `R` | Volver al encuadre inicial |
| `1` / `2` / `3` | Normales / albedo / difusa |
| `Escape` | Salir |

## Orden de construccion

El proyecto se armo en el orden del enunciado, y los modulos conservan esa
division.

**1. El raycaster.** Antes de que hubiera forma alguna ya estaba la
tuberia completa: `camera` genera un rayo por pixel, `ray` lo transporta,
`framebuffer` recibe el color y `renderer` recorre la imagen. El modo
`Normals` (tecla `1`) es literalmente esa etapa: pinta la normal de la cara
sin consultar ninguna luz.

**2. La forma.** `aabb` resuelve el *slab test* —interseca el rayo contra
los tres pares de planos de la caja y se queda con la interseccion de los
tres intervalos—, y `cuboid` agrega encima lo que el AABB no necesita saber
para decidir si hubo impacto pero el sombreado si: que cara se toco, hacia
donde mira su normal y que coordenada `uv` le corresponde.

**3. La difusa.** `light` implementa la ley de Lambert —el coseno entre la
normal y la direccion a la luz, recortado en cero— con caida cuadratica, y
`renderer` la suma sobre un piso de ambiente que conserva la silueta de lo
que ninguna luz alcanza.

## Decisiones que conviene conocer

- **El color vive en espacio lineal.** `Color::from_hex` decodifica de sRGB
  al entrar y `to_u32` vuelve a codificar al salir. Sumar y multiplicar luz
  sobre valores sRGB da resultados apagados y sucios.
- **La camara guarda cartesianas, no angulos.** El yaw y el pitch se
  derivan al orbitar. Una sola fuente de verdad —donde esta el ojo— en vez
  de dos representaciones que puedan desincronizarse.
- **Orbitar no mueve la geometria.** Lo unico que cambia es la base que
  `Camera::basis_change` aplica al rayo.
- **No hay rayos de sombra.** El cubo es convexo y es el unico objeto: no
  puede darse sombra a si mismo ni recibirla de nadie. Agregarlos ahora
  seria costo sin imagen.
- **Un rayo por pixel, sin antialiasing.** Las aristas salen duras a
  proposito; suavizarlas es muestrear varias veces por pixel y eso
  pertenece a otro paso.

## Estructura

```
src/
  ray.rs            origen + direccion, y el punto a distancia t
  hit.rs            el impacto, con la normal ya orientada contra el rayo
  ray_intersect.rs  el trait que cumple toda primitiva trazable
  aabb.rs           slab test: el corazon del raycaster
  cuboid.rs         cara, normal y uv sobre el resultado del slab test
  camera.rs         orbita, zoom y generacion del rayo primario
  color.rs          color lineal, con sRGB solo en los bordes
  light.rs          Lambert, atenuacion y ambiente
  scene.rs          el cubo, la luz y el impacto mas cercano
  framebuffer.rs    el lienzo y su volcado a PPM
  renderer.rs       el recorrido de la imagen y los tres modos
  main.rs           ventana, teclado y presentacion
  bin/render_ppm.rs render sin ventana
```

## Siguientes pasos

Rayos de sombra (exigen un segundo objeto para que se noten), un termino
especular, antialiasing por supermuestreo y texturas sobre el `uv` que la
primitiva ya calcula.
