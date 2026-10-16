/*{
    "DESCRIPTION": "Plasma : ondes de couleur qui se croisent (générateur d'exemple de Prism).",
    "CREDIT": "Prism contributors, MIT OR Apache-2.0",
    "ISFVSN": "2",
    "CATEGORIES": ["Generator"],
    "INPUTS": [
        { "NAME": "scale", "LABEL": "Échelle", "TYPE": "float", "MIN": 1.0, "MAX": 20.0, "DEFAULT": 6.0 },
        { "NAME": "speed", "LABEL": "Vitesse", "TYPE": "float", "MIN": 0.0, "MAX": 4.0, "DEFAULT": 1.0 },
        { "NAME": "tint", "LABEL": "Couleur", "TYPE": "color", "DEFAULT": [1.0, 0.6, 0.2, 1.0] },
        { "NAME": "center", "LABEL": "Centre", "TYPE": "point2D" }
    ]
}*/

void main() {
    vec2 p = isf_FragNormCoord * scale;
    float t = TIME * speed;
    vec2 c = center / RENDERSIZE * scale;
    float v = sin(p.x + t) + sin(p.y * 1.3 - t) + sin(distance(p, c) * 2.0 - t * 1.7);
    vec3 wave = 0.5 + 0.5 * cos(vec3(0.0, 2.1, 4.2) + v * 1.5);
    gl_FragColor = vec4(wave * tint.rgb, tint.a);
}
