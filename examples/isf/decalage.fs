/*{
    "DESCRIPTION": "Décalage : déplace les canaux rouge et bleu de part et d'autre (filtre d'exemple de Prism).",
    "CREDIT": "Prism contributors, MIT OR Apache-2.0",
    "ISFVSN": "2",
    "CATEGORIES": ["Filter"],
    "INPUTS": [
        { "NAME": "inputImage", "TYPE": "image" },
        { "NAME": "amount", "LABEL": "Écart", "TYPE": "float", "MIN": 0.0, "MAX": 0.1, "DEFAULT": 0.02 },
        { "NAME": "direction", "LABEL": "Sens", "TYPE": "long", "VALUES": [0, 1], "LABELS": ["Horizontal", "Vertical"], "DEFAULT": 0 },
        { "NAME": "pulse", "LABEL": "Battement", "TYPE": "bool", "DEFAULT": false }
    ]
}*/

void main() {
    vec2 axis = direction == 0 ? vec2(1.0, 0.0) : vec2(0.0, 1.0);
    float beat = pulse ? 0.5 + 0.5 * sin(TIME * 6.2831) : 1.0;
    vec2 shift = axis * amount * beat;
    vec4 middle = IMG_NORM_PIXEL(inputImage, isf_FragNormCoord);
    float red = IMG_NORM_PIXEL(inputImage, isf_FragNormCoord - shift).r;
    float blue = IMG_NORM_PIXEL(inputImage, isf_FragNormCoord + shift).b;
    gl_FragColor = vec4(red, middle.g, blue, middle.a);
}
