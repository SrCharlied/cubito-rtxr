# cubito-rtxr

Raytracer minimo en Rust con **camara orbital**. Dos escenas, que son la
misma figura leida de dos maneras:

- **Jaula difusa** (`J`) — el teseracto en **luz difusa pura**: doce
  barras sobre las aristas del cubo exterior y un nucleo macizo dentro,
  sobre un piso. Lambert y sombras, nada mas. Es la escena del enunciado.
- **Teseracto** (`T`) — el mismo objeto como el cubo cosmico: cascaron de
  vidrio azul, nucleo incandescente, marco de aristas encendido, halo,
  charco de luz y sombra tenida.

Sin reflexion y sin refraccion.

## Como correrlo

```bash
cargo run --release              # ventana interactiva
cargo test                       # 118 pruebas, sin ventana
```

Render sin ventana, util para dejar evidencia o para verificar en una
maquina sin servidor grafico:

```bash
cargo run --release --bin render_ppm -- salida.ppm --escena teseracto --yaw 55 --pitch -18
cargo run --release --bin render_ppm -- jaula.ppm --escena jaula --yaw 35
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
| `T` / `J` | Teseracto / jaula difusa |
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
bajar bajo la losa. En la jaula las sombras son duras y neutras, como
corresponde a un opaco, y las piezas se ocluyen entre si; en el teseracto
la sombra sale azul.

**6. La jaula**, que es el teseracto devuelto a la luz difusa pura. Ver
abajo por que no basta con apagarle los efectos.

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

## Por que la jaula existe

La pregunta natural es: ¿y si al teseracto simplemente le apagamos todo y
lo dejamos en difusa pura? No funciona, y el motivo es mas profundo que
perder el brillo.

Un cubo opaco dentro de otro cubo opaco **no se ve**. Sin transmision, el
cascaron tapa el nucleo y lo que queda es un hexagono azul liso,
indistinguible de un cubo cualquiera. Se puede comprobar sin tocar codigo:
abra el teseracto y presione `2`, el modo albedo, que es opaco a
proposito. Lo que se pierde al apagar el vidrio no es el brillo: es la
estructura, porque el «cubo dentro del cubo» solo existia gracias a que se
podia ver a traves del cascaron.

La jaula resuelve eso dejando de pedirle al material lo que puede dar la
geometria: el cubo exterior deja de ser una caja y pasa a ser **doce
barras** sobre sus aristas. El nucleo se ve por los huecos. El precio es
que se lee como un modelo fisico y no como el objeto de la pelicula; a
cambio, todo lo que decide un pixel ahi es la ley de Lambert y si algo se
interpone.

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
- **Las dos escenas comparten figura, tamano y posicion.** Alternar entre
  ellas con `J` y `T` compara materiales sin que la silueta se mueva de
  sitio; es la forma mas rapida de ver que aporta cada capa.
- **Todo flota sobre el piso.** Apoyado, la sombra nace debajo del objeto
  y queda escondida por el propio objeto justo donde se la quiere ver.
- **Varias luces siguen siendo luz difusa.** La jaula usa dos, una calida
  y una fria: lo que define a la difusa es la ley de Lambert, no cuantas
  lamparas hay. Con una sola, la mitad de las barras cae a puro ambiente y
  la jaula se lee plana.
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
  scene.rs          las escenas, el piso y el impacto mas cercano
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
