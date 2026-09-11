#!/usr/bin/env python3
"""Generador de las texturas del teseracto.

    python tools/generar_texturas.py

Produce dos PPM en `assets/`, que son los que carga la escena texturizada.
Los archivos se versionan en el repositorio y este script se conserva como
su fuente reproducible.

Por que generarlas y no descargarlas, en orden de importancia:

1. **Procedencia.** Una textura generada aqui tiene autoria clara. Una
   descargada arrastra una licencia que hay que rastrear y acreditar.
2. **Reproducibilidad.** Con semilla fija, cualquiera regenera bytes
   identicos desde un clon limpio, sin red y sin dependencias: este script
   usa solo la biblioteca estandar.
3. **Sin costura.** El ruido es periodico por construccion, asi que el
   patron repite sin junta visible. Eso importa aqui mas que de costumbre:
   el `uv` del cuboide se calcula por cara, de modo que las seis caras
   muestrean la misma imagen y cualquier junta se veria seis veces, justo
   sobre las aristas.

# Formato

PPM binario (P6), el mismo que el renderer ya sabe escribir. Es
descomprimido y pesado, pero a cambio ni este script ni el renderer
necesitan un codificador de PNG. Son tres bytes por pixel detras de una
cabecera de texto.

# Espacio de color

Los bytes que se escriben son **sRGB**, como los de cualquier imagen, y el
renderer los decodifica a lineal al cargarlos. Las texturas de aqui son
mascaras en escala de grises: el color se lo pone el material, para que la
misma imagen pueda re-tenirse sin regenerarla.
"""

import math
import os
import struct

# Cuadradas y potencia de dos, como las del proyecto de referencia.
LADO = 256

# Semillas fijas. Cambiarlas cambia el patron; dejarlas quietas es lo que
# hace que dos clones produzcan bytes identicos.
SEMILLA_VETAS = 20260910
SEMILLA_PLASMA = 771013

# Cuantas celdas tiene la red de facetas. Pocas y grandes: se busca un
# cristal de facetas anchas, no un vitral.
CELDAS_DE_FACETA = 14

# Ancho de la veta como fraccion de la distancia entre celdas.
ANCHO_DE_VETA = 0.055


# --------------------------------------------------------------- ruido


def hash01(x, y, semilla):
    """Hash entero a 0.0..1.0. Determinista y sin estado."""
    n = (x * 374761393 + y * 668265263 + semilla * 1442695041) & 0xFFFFFFFF
    n = ((n ^ (n >> 13)) * 1274126177) & 0xFFFFFFFF
    n ^= n >> 16

    return (n & 0xFFFFFF) / float(0xFFFFFF)


def suavizar(t):
    """Interpolacion suave: derivada nula en los extremos, asi que las
    celdas empalman sin marcar la rejilla."""
    return t * t * (3.0 - 2.0 * t)


def ruido(u, v, celdas, semilla):
    """Ruido de valor **periodico** sobre una rejilla de `celdas` por lado.

    La periodicidad sale de envolver los indices con el modulo: la celda
    que sigue a la ultima es otra vez la primera. Eso es lo que hace que la
    textura repita sin costura.
    """
    x = u * celdas
    y = v * celdas
    x0 = math.floor(x)
    y0 = math.floor(y)
    tx = suavizar(x - x0)
    ty = suavizar(y - y0)

    def esquina(dx, dy):
        return hash01((x0 + dx) % celdas, (y0 + dy) % celdas, semilla)

    arriba = esquina(0, 0) + (esquina(1, 0) - esquina(0, 0)) * tx
    abajo = esquina(0, 1) + (esquina(1, 1) - esquina(0, 1)) * tx

    return arriba + (abajo - arriba) * ty


def fbm(u, v, celdas, octavas, semilla):
    """Suma de octavas. Cada una duplica la rejilla y halva la amplitud.

    Duplicar mantiene la periodicidad: si la rejilla base envuelve en
    `celdas`, la de `2 x celdas` envuelve tambien en el mismo tile.
    """
    total = 0.0
    amplitud = 1.0
    normalizador = 0.0

    for octava in range(octavas):
        total += ruido(u, v, celdas * (2**octava), semilla + octava * 7919) * amplitud
        normalizador += amplitud
        amplitud *= 0.5

    return total / normalizador


def cresta(u, v, celdas, octavas, semilla):
    """Ruido de **cresta**: filamentos finos en vez de manchas.

    Sale de doblar el ruido sobre si mismo con `1 - |2n - 1|`. Donde el
    ruido de valor cruza la mitad, el doblez produce un maximo agudo, y lo
    que era una transicion suave entre mancha clara y oscura se convierte
    en una linea brillante. Elevarlo al cuadrado afina esas lineas.

    Es lo que separa una energia de una vineta: el ruido de valor a secas
    da nubes, y las nubes no se leen como plasma.
    """
    total = 0.0
    amplitud = 1.0
    normalizador = 0.0

    for octava in range(octavas):
        n = ruido(u, v, celdas * (2**octava), semilla + octava * 7919)
        filamento = 1.0 - abs(2.0 * n - 1.0)

        total += filamento * filamento * amplitud
        normalizador += amplitud
        amplitud *= 0.5

    return total / normalizador


def distancia_toroidal(au, av, bu, bv):
    """Distancia en un tile que se repite: el borde derecho es vecino del
    izquierdo. Sin esto, las celdas de la red se cortarian en los bordes de
    la imagen y la junta se veria sobre cada arista del cubo."""
    du = abs(au - bu)
    dv = abs(av - bv)
    du = min(du, 1.0 - du)
    dv = min(dv, 1.0 - dv)

    return math.sqrt(du * du + dv * dv)


# ------------------------------------------------------------ texturas


def generar_vetas():
    """Red de facetas del cristal: celdas anchas con las juntas encendidas.

    Es ruido de Worley leido por la **diferencia entre las dos distancias
    mas cercanas**. Ese valor es cero justo sobre la frontera entre dos
    celdas y crece hacia el centro de cada una, asi que invertirlo dibuja
    exactamente la red de juntas, sin tener que trazar ninguna linea.
    """
    puntos = []
    for i in range(CELDAS_DE_FACETA):
        puntos.append((hash01(i, 0, SEMILLA_VETAS), hash01(0, i, SEMILLA_VETAS + 1)))

    pixeles = bytearray()

    for fila in range(LADO):
        v = (fila + 0.5) / LADO
        for columna in range(LADO):
            u = (columna + 0.5) / LADO

            primera = segunda = 10.0
            for pu, pv in puntos:
                d = distancia_toroidal(u, v, pu, pv)
                if d < primera:
                    segunda = primera
                    primera = d
                elif d < segunda:
                    segunda = d

            # 1 sobre la junta, 0 hacia el centro de la celda.
            junta = 1.0 - min(1.0, (segunda - primera) / ANCHO_DE_VETA)
            junta = suavizar(junta)

            # Un piso tenue con grano para que el interior de cada faceta no
            # quede muerto: el vidrio tiene cuerpo, no es un vacio con
            # lineas encima.
            grano = 0.06 + 0.10 * fbm(u, v, 6, 3, SEMILLA_VETAS + 31)

            valor = max(grano, junta)
            byte = int(round(min(1.0, valor) * 255))
            pixeles += bytes((byte, byte, byte))

        if fila % 64 == 0:
            print(f"  vetas {100 * fila // LADO:3d}%")

    return pixeles


def generar_plasma():
    """Energia del nucleo: ruido de varias octavas concentrado al centro.

    El `uv` del cuboide va de 0 a 1 sobre cada cara, asi que la
    concentracion radial deja cada cara del nucleo con su propio foco. Es lo
    que evita que el nucleo se lea como seis rectangulos blancos planos.
    """
    pixeles = bytearray()

    for fila in range(LADO):
        v = (fila + 0.5) / LADO
        for columna in range(LADO):
            u = (columna + 0.5) / LADO

            # Distancia al centro de la cara, normalizada al borde.
            du = (u - 0.5) * 2.0
            dv = (v - 0.5) * 2.0
            radio = min(1.0, math.sqrt(du * du + dv * dv))
            foco = 1.0 - suavizar(radio)

            filamentos = cresta(u, v, 5, 5, SEMILLA_PLASMA)

            # Dos factores que se multiplican: donde esta la energia —el
            # foco— y de que esta hecha —los filamentos—. El piso de cada
            # uno impide que las esquinas se apaguen del todo y que entre
            # filamento y filamento quede un hueco negro: es una fuente
            # encendida, no una lampara con vineta.
            valor = (0.30 + 0.70 * foco) * (0.45 + 0.55 * filamentos)

            byte = int(round(min(1.0, valor) * 255))
            pixeles += bytes((byte, byte, byte))

        if fila % 64 == 0:
            print(f"  plasma {100 * fila // LADO:3d}%")

    return pixeles


# ---------------------------------------------------------------- salida


def escribir_ppm(ruta, pixeles):
    cabecera = f"P6\n{LADO} {LADO}\n255\n".encode("ascii")

    with open(ruta, "wb") as archivo:
        archivo.write(cabecera)
        archivo.write(pixeles)

    print(f"  {ruta}  {len(cabecera) + len(pixeles)} bytes")


def main():
    raiz = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    destino = os.path.join(raiz, "assets")
    os.makedirs(destino, exist_ok=True)

    print(f"generando texturas de {LADO} x {LADO} en {destino}")

    escribir_ppm(os.path.join(destino, "vetas.ppm"), generar_vetas())
    escribir_ppm(os.path.join(destino, "plasma.ppm"), generar_plasma())

    print("listo")


if __name__ == "__main__":
    main()
