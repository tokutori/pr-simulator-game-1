# 座標系・単位契約

すべてSI単位を使用する。距離m、速度m/s、角度rad、角速度rad/s、質量kg、力N、moment N mとする。
内部数値はf64を基本とし、描画境界でのみf32等へ変換する。

## 座標系

| 系 | x | y | z | 原点 |
|---|---|---|---|---|
| Navigation (NED) | 北 | 東 | 下 | 発進地点の水平位置と基準湖面の交点 |
| Body (FRD) | 前 | 右 | 下 | パイロットを除く機体の固定重心 $O$ |
| Three.js world | 東 | 上 | 南 | NEDと同一 |

地理原点の緯度・経度・標高は実データ選定時に確定し、world metadataへ保存する。
発進時の重心高度を $h$ とするとNED座標の下向き成分は $-h$ である。
発進地点という名称にplatform上面の高度を含めない。
楕円体高と標高の違い、geoid補正、湖面基準、基準epochはoffline処理で明記する。

PointとVector、BodyFrameとNedFrameを型で区別する。
Point−PointはVector、Point+VectorはPointである。異なるframeの暗黙加算を禁止する。
機体構造の点はdatumからの固定offsetで表す。パイロットの前後移動に伴い合成重心のbody座標は変化する。
運動学と構造datum $O$ を基準とする積分状態は `pilot-motion.md` に従う。燃料消費は初期モデルの対象外である。

## 姿勢

quaternionはHamilton積、scalar-first $(w,x,y,z)$、単位長、body-to-NEDの能動回転とする。
記号 $q_{NB}$ と $R_{NB}$ はbodyのベクトルをNEDへ変換する。

```math
v^N=R_{NB}v^B,\qquad v^B=R_{NB}^{T}v^N
```

角速度 $(p,q,r)$ はbody表現、各正軸に対する右手系とする。
正rollは右翼下げ、正pitchは機首上げ、正yawは右旋回方向である。
Euler angleは表示用に導出し、積分stateに保存しない。
quaternion補間は最短経路を採用し、$q$ と $-q$ の同一姿勢を考慮する。

## 描画変換

```math
\begin{bmatrix}X\\Y\\Z\end{bmatrix}
=C\begin{bmatrix}N\\E\\D\end{bmatrix},\qquad
C=\begin{bmatrix}0&1&0\\0&0&-1\\-1&0&0\end{bmatrix}
```

描画modelのローカル軸も同じCで変換済みなら姿勢は $C R_{NB} C^T$ である。
asset固有の前方軸が異なる場合はimport時の固定変換を別に記録する。
Three.js quaternionの引数順 $(x,y,z,w)$ への変換はadapterのみで行う。

## 風向

WindFieldは空気の移動先を表す速度ベクトルである。
気象データの風向は通常「吹いてくる方向」であるため、source定義を確認してofflineで変換する。
NEDでは上昇流の $W_D$ は負である。
