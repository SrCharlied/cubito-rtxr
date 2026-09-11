# cubito-rtxr

Raytracer minimo en Rust con **camara orbital**. Dos escenas:

- **Teseracto** — el cubo cosmico flotando sobre un piso: cascaron de
  vidrio azul, nucleo incandescente, marco de aristas encendido, halo,
  charco de luz y sombra tenida.
- **Cubo mate** — el cubo opaco sobre el mismo piso, con **iluminacion
  difusa pura**: Lambert, su sombra y nada mas. Ni emision, ni
  transmision, ni halo. Es la escena del enunciado, y la referencia contra
  la cual se lee el teseracto.

Sin reflexion y sin refraccion.

## Como correrlo

```bash
cargo run --release              # ventana interactiva
cargo test                       # 110 pruebas, sin ventana
```

Render sin ventana, util para dejar evidencia o para verificar en una
maquina sin servidor grafico:

```bash
cargo run --release --bin render_ppm -- salida.ppm --escena teseracto --yaw 55 --pitch -18
cargo run --release --bin render_ppm -- cubo.ppm --escena cubo --modo normales
```

El PPM (P6) lo abre cualquier visor decente y no obliga a arrastrar un
codificador de PNG como dependencia.

## Controles

| Tecla | Efecto |
| --- | --- |
| Flechas | Orbitar |
| `W` / `S` / rueda | Acercar y alejar |
| `R` | Volver al encuadre inicial |
| `T` / `C` | Teseracto / cubo mate |
| `1` / `2` / `3` | Normales / albedo / difusa |
| `Escape` | Salir |

## Orden de construccion

**1. El raycaster.** Antes de que hubiera forma alguna ya estaba la
tuberia completa: `camera` genera un rayo por pixel, `ray` lo transporta,
`framebuffer` recibe el color y `renderer` recorre la imagen. El modo
`Normals` (tecla `1`) es literalmente esa etapa.

**2. La forma.** `aabb` resuelve el *slab test* —interseca el rayo contra
los tres pares de planos de la caja y se queda con la interseccion de los
tres intervalos—, y `cuboid` agrega encima lo que el AABB no necesita saber
para decidir si hubo impacto pero el sombreado si: que cara se toco, hacia
donde mira su normal y que coordenada `uv` le corresponde.

**3. La difusa.** `light` implementa la ley de Lambert —el coseno entre la
normal y la direccion a la luz, recortado en cero— con caida cuadratica,
sobre un piso de ambiente que conserva la silueta.

**4. El teseracto.** `material` agrega emision, transmision, absorcion,
resplandor de volumen y marco de aristas; `bloom` agrega el halo.

**5. El piso**, en las dos escenas. Es tambien un `Cuboid`, solo que
aplastado: no hizo falta primitiva nueva. Con el aparecen los rayos de
sombra, que hasta aqui no tenian sentido —un objeto convexo y solo no
puede darse sombra a si mismo—, y con ellos el tope de camara que impide
bajar bajo la losa. En el cubo mate la sombra es dura y neutra, como
corresponde a un opaco; en el teseracto sale azul.

## Por que el teseracto se ve asi

**El cubo dentro del cubo no es decorado.** Es la proyeccion clasica del
hipercubo de cuatro dimensiones: al proyectarlo a tres dimensiones, la
celda «lejana» en la cuarta queda dentro de la cercana. Como la primitiva
del proyecto es un cuboide alineado a los ejes y no hay rotaciones de por
medio, dos AABB concentricos **son** exactamente esa proyeccion.

Siete piezas, y ninguna es un reflejo:

| Pieza | Que hace | Donde vive |
| --- | --- | --- |
| Transmision | El rayo sigue derecho a traves del cascaron y ve lo de atras | `renderer::atravesar` |
| Absorcion (Beer-Lambert) | Tine el vidrio de azul comiendose el rojo, mas cuanto mas medio se cruza | `material::beer` |
| Resplandor de volumen | Luz por unidad de longitud: llena la masa en vez de dejar una jaula de alambre | `Material::inner_glow` |
| Marco de aristas | Del `uv` que la primitiva ya calculaba: `min(u, 1-u, v, 1-v)` | `Material::edge_factor` |
| Halo | Derrama lo que paso del blanco, **antes** de recortar a 8 bits | `bloom` |
| Charco de luz | Una segunda luz en el centro del objeto, que es la que pinta el piso de azul | `scene::teseracto` |
| Sombra tenida | El rayo de sombra acumula transmision y absorcion en vez de devolver si/no | `renderer::transmitancia` |

## Decisiones que conviene conocer

- **El color vive en espacio lineal y sin acotar.** `from_hex` decodifica
  de sRGB al entrar; el recorte ocurre una sola vez, en `to_u32`. El
  framebuffer guarda las dos vistas porque el bloom tiene que leer el rango
  completo antes de que el recorte lo tire: un pixel a cuatro veces el
  blanco y otro justo en el blanco se empacan igual, y el derrame es la
  unica pista que queda de la diferencia.
- **El medio solo cobra el tramo que se recorre.** Cuando el nucleo
  interrumpe el rayo a mitad del cascaron, la absorcion se cobra hasta ahi
  y no sobre la cuerda completa. Por eso `traza` devuelve tambien la
  distancia: sin ese dato el nucleo sale oscuro y sobre-tenido.
- **No hay refraccion.** Doblar el rayo exige Snell mas Fresnel, y sin
  reflexiones que lo acompanen un cubo que deforma lo de atras pero no
  devuelve nada del entorno se ve mas raro que uno que no deforma. El
  tenido volumetrico hace el trabajo.
- **La camara guarda cartesianas, no angulos.** El yaw y el pitch se
  derivan al orbitar: una sola fuente de verdad en vez de dos
  representaciones que puedan desincronizarse.
- **Orbitar no mueve la geometria.** Lo unico que cambia es la base que
  `Camera::basis_change` aplica al rayo.
- **La sombra es un color, no una bandera.** El rayo de sombra atraviesa
  lo que sea transmisivo acumulando transmision y absorcion, asi que la
  sombra del teseracto sale azul, con el nucleo opaco recortado en oscuro
  dentro de ella. Un `bool` daria un parche gris.
- **Una de las dos luces no proyecta sombra, a proposito.** La del
  teseracto nace en su centro, que esta dentro del nucleo opaco: un rayo
  de sombra hacia ella daria siempre bloqueado y el piso quedaria negro
  bajo el objeto que se supone que lo ilumina. El punto es una
  simplificacion de una fuente extendida, no una lampara escondida en una
  caja, y `Light::casts_shadows` lo dice explicito.
- **La camara no baja del piso.** Sin el tope, orbitar hacia abajo mete el
  ojo bajo la losa y la pantalla se pone negra, que nadie lee como «estoy
  debajo del piso». Se corrige la **posicion** despues de mover y no el
  angulo antes, porque el pitch admisible depende del radio y un tope
  calculado una vez queda flojo tras un zoom.
- **Los dos cubos flotan sobre el piso.** Apoyados, su sombra nace debajo
  y queda escondida por el propio objeto justo donde se la quiere ver. Y
  compartiendo posicion, alternar entre las escenas con `C` y `T` compara
  materiales sin que la silueta se mueva de sitio.
- **Un rayo por pixel, sin antialiasing.** Las aristas salen duras.

## Estructura

```
src/
  ray.rs            origen + direccion, y el punto a distancia t
  hit.rs            el impacto, con la normal ya orientada contra el rayo
  ray_intersect.rs  el trait que cumple toda primitiva trazable
  aabb.rs           slab test: el corazon del raycaster
  cuboid.rs         cara, normal, uv y grosor del medio
  camera.rs         orbita, zoom y generacion del rayo primario
  color.rs          color lineal, con sRGB solo en los bordes
  material.rs       albedo, emision, transmision, absorcion, marco
  light.rs          Lambert, atenuacion y ambiente
  scene.rs          las dos escenas, el piso y el impacto mas cercano
  bloom.rs          halo por umbral y desenfoque de caja acumulada
  framebuffer.rs    lienzo lineal + vista empacada, y volcado a PPM
  renderer.rs       el recorrido de la imagen y los tres modos
  main.rs           ventana, teclado y presentacion
  bin/render_ppm.rs render sin ventana
```

## Siguientes pasos

Pulso animado del nucleo, refraccion con Fresnel, antialiasing por
supermuestreo, y sombras suaves muestreando una luz de area en vez de un
punto.
