uniform sampler2D ShadowMap;
uniform mat4 ShadowProjection;
uniform float ShadowEnabled;
float unpackDepth(vec4 rgba) {
    return dot(rgba, vec4(1.0, 1.0/255.0, 1.0/65025.0, 1.0/16581375.0));
}
float sunVisibility(vec3 p, vec3 n, vec3 sun) {
    if (ShadowEnabled < 0.5) return 1.0;
    vec4 projected = ShadowProjection * vec4(p, 1.0);
    vec3 q = projected.xyz / projected.w * 0.5 + 0.5;
    if (q.x <= 0.01 || q.x >= 0.99 || q.y <= 0.01 || q.y >= 0.99 || q.z >= 1.0 || q.z <= 0.0) return 1.0;
    float bias = max(0.0007, 0.0020 * (1.0 - max(0.0, dot(n,sun))));
    float visibility = 0.0;
    for (int x=-1; x<=1; x++) {
        for (int y=-1; y<=1; y++) {
            float depth = unpackDepth(texture2D(ShadowMap, q.xy + vec2(float(x),float(y))/1024.0));
            visibility += q.z-bias <= depth ? 1.0 : 0.0;
        }
    }
    return visibility / 9.0;
}
