# BPG-043 contactの数値精度

## 条件と分離

`contact::tests::numerical`は既存contact試験の架空AircraftModel・tail incidence・geometry構築を再利用する。
外部荷重0、pilot静止、定姿勢、NED水平速度$(6,8)$ m/s、初期down速度$w_0=2$ m/s、$g=9.81$ m/s²とする。
接触点はdatumとbody down offset 0.4 mの二点であり、後者が最初に静水面へ到達する。
有効高さ$h$はそのoffsetを除いた初期clearanceである。
時刻$0.4+(0.125,0.375,0.625,0.875)\times0.01$ sを選び、$h=w_0t_c+gt_c^2/2$から初期条件を定める。
計算する参照時刻は桁落ちを避けた$t_c=2h/(w_0+\sqrt{w_0^2+2gh})$である。

比較はf64・100/200/400 Hzのtest-only driverとし、最大区間は1 sとする。
製品tick・record周波数・公開API・機体値は変更しない。区間indexとfractionから、そのdriverのdtで物理時刻を計算する。
200/400 Hzの区間indexを100 Hzの保存recordへ変換しない。
このgeneric ballistic fixtureは空力providerを使用せず、hybridの範囲外軌道を外挿しない。

1. 解析式のendpointを内部`detect_flight_state_water_contact`へ渡し、event補間の誤差だけを検査する。
2. 同じ条件を既存`advance`でRK4積分し、全endpointを解析式と比較する。
   そのcontact結果と1の差を、積分・累積丸めに由来する追加誤差として個別に判定する。

## 事前の誤差上限

定姿勢の鉛直軌道$z(t)=-h+w_0t+gt^2/2$に対し、区間$[a,b]$の線形chordの超過は
$L(t)-z(t)=g(t-a)(b-t)/2\le g\,dt^2/8$である。
$z'(t)\ge w_0>0$を用い、eventの時刻誤差を以下で判定する。

```math
0\le t_c-\hat t_c\le \frac{g\,dt^2}{8w_0}
```

実APIは補間枝ごとの内部候補で区間を隔離して最大48回二分するため、二分区間の時間幅は最大$dt/2^{48}$である。
f64によるmidpoint停止とplane分類の丸めを別の余裕へ加える。
この許容差とfixtureは実行前に固定し、結果に対する事後fitを行わない。

丸め余裕は$128(N+1)\epsilon S$とする。解析endpointは$N=0$、RK4 endpointは最大区間のstep数$N=f$、
$f$は100/200/400、$\epsilon$はf64 epsilonである。
位置scaleは初期datum高さ・contact offset・1 sの並進距離と重力変位から、
速度scaleは初速と1 sの重力速度増分から定める。姿勢・rateのscaleは1とする。
128はendpoint計算・検査で用いる演算scaleへの事前余裕であり、厳密な丸め誤差証明とは区別する。
時刻には位置余裕/$w_0$、二分区間幅、$8\epsilon\max(t_c,dt)$を加える。
RK4と解析endpointのcontact時刻差は双方の位置余裕/$w_0$と二分・時刻丸めの和で判定する。

scoreは公開`course_distance_score`のnorth courseを使用する。course/cross/netはそれぞれ$(6,8,10)t_c$ mである。
各誤差上限は対応する水平速度と時刻誤差上限の積に、位置の演算余裕を加える。
100/200/400相互差は双方の量別上限の和で判定する。接触の区間内phaseは刻みごとに変わるため、
無条件の単調改善・一定収縮率を要求しない。

## 同時刻stateと検証範囲

実`FlightStateContactSample`のinterval index・fraction・contact point・stateを使用する。
水平位置と鉛直速度は同じ$\hat t_c$へ対応し、定姿勢・body rate・pilot位置/速度を保持する。
鉛直位置は線形chord上の水面接触値であり、解析的ballistic位置との差を補間誤差として扱う。
内部検出が返さないTail controlsを生成しない。実tail tickのincidence保持とrecord終端は
`off_grid_tail_contact_seals_record_and_score_at_the_first_fractional_time`で接続を検査する。
接触後state・実機精度・新hybridのevent成立率はこの試験の対象外である。
結果とcommit/toolchain/環境は検査実行時に報告する。

## Off-grid接触の隔離と丸め

通常slerpでは各点のdownは位置の線形項とsin/cosの和となる。shortest arcの物理角は最大πであり、
微分の内部根は高々二つとなる。近角nlerpは正規化前quaternionが一次式であり、
その正の二次normで分母を払った接触関数Pは高々三次となる。P'の二次根で符号区間を隔離する。
同じmath moduleのshortest符号と枝判定を用い、補間経路・演算順・最終正規化を変更しない。
二次根にはno_stdの固定`polycool`を使用する。三次式の符号交差だけで接線を探索する方式は用いない。

候補の分類と二分には実際の位置・quaternion補間で再評価したdownを使用する。
内部候補の接線分類だけに、$128\epsilon S+\lVert r\rVert_1 G$の丸め距離予算を適用する。
$S=\max(|d_0|,|d_1|)+\lVert r\rVert_1$は位置・回転点の演算scaleであり、128は係数・正規化・回転評価の演算余裕である。
通常slerpのbasisをa,bとすると、Gram欠損$G=|a\cdot a-1|+|b\cdot b-1|+|a\cdot b|$を加える。
これは正規化前normの1からの差による回転downの誤差を表す。nlerpではG=0とする。
この予算はf64/libmによる数値解像度の契約であり、全実入力の厳密丸め誤差証明や固定mの侵入許容値とは区別する。
幾何scaleの回帰検査は1e-300〜1e150を含む。subnormal下端の丸め予算は未検証である。
内部候補の正負がこの丸め帯に入る場合は、接線とごく浅い侵入を区別せず候補時刻で分類する。
端点の接触と、この丸め帯を超えた通常交差の二分は従来のdown>=0で分類する。

二分探索幅はevent時刻の総誤差と区別する。単純根には局所傾きmを介した距離予算/mが作用する。
接線近傍では曲率により平方根の増幅が生じ得るため、任意接線に一様なfraction精度を主張しない。
各点は高々三つの区間端と、必要な一回の最大48回精密化を評価する。点数に線形、追加allocationは0である。
差分・係数・実評価の非有限はMath causeとして返し、候補を得た後も全点の失敗を評価する。
