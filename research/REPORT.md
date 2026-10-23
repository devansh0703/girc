# GIRC — the report

Every figure in this document is generated from a committed artifact by `tools/generate_report.py`, which reads the benchmark report, the trained model, the training records and the spread study. A figure that is not in one of those files is a figure that was invented, and the CI check compares the documents against the benchmark for exactly that reason.

The report is generated, not written, so it cannot drift from the run that produced it.

## The result

Over twelve Xiph.Org CIF sequences, ten frames each, at four quantization parameters, with the margin at 50, the inferred mode is **−0.236% on the luma plane** and **−0.149% on the whole frame**.

The learned predictor beats the constant one, which always answers the plain DCT-II and needs no training and no model:

| Predictor | Mean BD-rate, luma | Better on |
|---|---|---|
| Constant, always the plain DCT-II | −0.120% | 10 of 12 |
| **The shipped learned model** | **−0.236%** | 11 of 12 |

The constant predictor's whole-frame figure is not published, because the benchmark report holds its luma figure only and a number that is not in the report is a number somebody made up. The whole-frame axis is the luma figure of the learned model, which is what a deployment would save.

The gap between them is 0.116 percentage points of BD-rate, which is about half the mechanism's own effect.

## The margin sweep

The margin is the amount of rate-distortion cost the encoder is willing to give up to avoid signalling the transform type. It is the mechanism's only free parameter, and sweeping it shows the whole trade.

<figure class="chart">
  <figcaption>
    <span>The margin sweep: the rate the encoder saves against the distortion it pays</span>
    <span class="src">research/benchmark/report-v9-joint.json · margin_sweep</span>
  </figcaption>
  <div class="plot">
    <svg viewBox="0 0 900 360" role="img" aria-label="Mean Bjontegaard delta rate against the explicit mode, by margin, over twelve sequences: the best point is a margin of 50 at minus 0.236 percent">
    <line x1="70.0" y1="280.0" x2="860.0" y2="280.0" stroke="var(--grid)" stroke-width="1.0" />
    <text x="60.0" y="284.0" font-size="11" fill="var(--ink-faint)" text-anchor="end" font-weight="400">-0.3</text>
    <line x1="70.0" y1="220.0" x2="860.0" y2="220.0" stroke="var(--grid)" stroke-width="1.0" />
    <text x="60.0" y="224.0" font-size="11" fill="var(--ink-faint)" text-anchor="end" font-weight="400">-0.1</text>
    <line x1="70.0" y1="160.0" x2="860.0" y2="160.0" stroke="var(--grid)" stroke-width="1.0" />
    <text x="60.0" y="164.0" font-size="11" fill="var(--ink-faint)" text-anchor="end" font-weight="400">0.1</text>
    <line x1="70.0" y1="100.0" x2="860.0" y2="100.0" stroke="var(--grid)" stroke-width="1.0" />
    <text x="60.0" y="104.0" font-size="11" fill="var(--ink-faint)" text-anchor="end" font-weight="400">0.4</text>
    <line x1="70.0" y1="40.0" x2="860.0" y2="40.0" stroke="var(--grid)" stroke-width="1.0" />
    <text x="60.0" y="44.0" font-size="11" fill="var(--ink-faint)" text-anchor="end" font-weight="400">0.6</text>
    <line x1="70.0" y1="196.0" x2="860.0" y2="196.0" stroke="var(--line-strong)" stroke-width="1.4" />
    <text x="860.0" y="188.0" font-size="11.5" fill="var(--ink-dim)" text-anchor="end" font-weight="400">the explicit baseline</text>
    <polyline points="70.0,210.5 267.5,226.7 465.0,256.8 662.5,195.4 860.0,63.2" fill="none" stroke="var(--accent)" stroke-width="2.2" />
    <circle cx="70.0" cy="210.5" r="4.5" fill="var(--accent)" />
    <text x="70.0" y="198.5" font-size="11.5" fill="var(--ink)" text-anchor="middle" font-weight="500">-0.056</text>
    <text x="70.0" y="300.0" font-size="11" fill="var(--ink-faint)" text-anchor="middle" font-weight="400">0</text>
    <circle cx="267.5" cy="226.7" r="4.5" fill="var(--accent)" />
    <text x="267.5" y="214.7" font-size="11.5" fill="var(--ink)" text-anchor="middle" font-weight="500">-0.119</text>
    <text x="267.5" y="300.0" font-size="11" fill="var(--ink-faint)" text-anchor="middle" font-weight="400">25</text>
    <circle cx="465.0" cy="256.8" r="4.5" fill="var(--accent)" />
    <text x="465.0" y="244.8" font-size="11.5" fill="var(--ink)" text-anchor="middle" font-weight="500">-0.236</text>
    <text x="465.0" y="300.0" font-size="11" fill="var(--ink-faint)" text-anchor="middle" font-weight="400">50</text>
    <circle cx="662.5" cy="195.4" r="4.5" fill="var(--accent)" />
    <text x="662.5" y="183.4" font-size="11.5" fill="var(--ink)" text-anchor="middle" font-weight="500">+0.002</text>
    <text x="662.5" y="300.0" font-size="11" fill="var(--ink-faint)" text-anchor="middle" font-weight="400">100</text>
    <circle cx="860.0" cy="63.2" r="4.5" fill="var(--accent)" />
    <text x="860.0" y="51.2" font-size="11.5" fill="var(--ink)" text-anchor="middle" font-weight="500">+0.515</text>
    <text x="860.0" y="300.0" font-size="11" fill="var(--ink-faint)" text-anchor="middle" font-weight="400">200</text>
    <text x="465.0" y="290.8" font-size="11.5" fill="var(--sev-ok)" text-anchor="middle" font-weight="400">the optimum on this content</text>
    <text x="14.0" y="24.0" font-size="11.5" fill="var(--ink-dim)" text-anchor="start" font-weight="400">BD-rate, %</text>
    <text x="465.0" y="324.0" font-size="11.5" fill="var(--ink-dim)" text-anchor="middle" font-weight="400">margin, in rate-distortion cost units</text>
    <text x="70.0" y="24.0" font-size="12" fill="var(--ink-faint)" text-anchor="start" font-weight="400">negative is smaller than the explicit mode</text>
    </svg>
  </div>
</figure>

| Margin | Mean BD-rate, luma | Prediction mismatch | Better on | Reading |
|---|---|---|---|---|
| 0 | −0.056 | 46.4% | 12 of 12 | No margin: the encoder always takes the optimal transform |
| 25 | −0.119 | 44.6% | 10 of 12 | The margin starts to pay |
| 50 | −0.236 | 42.6% | 11 of 12 | The optimum on this content |
| 100 | +0.002 | 37.9% | 10 of 12 | Past the optimum |
| 200 | +0.515 | 30.6% | 6 of 12 | Distortion dominates |

The margin is what turns a wrong prediction into a free one: at a margin of zero the encoder always takes the optimal transform, and the mismatch rate is at its highest, because every mismatch is an override the encoder has to pay for.

<figure class="chart">
  <figcaption>
    <span>How often the prediction is wrong, and how the margin buys it back</span>
    <span class="src">research/benchmark/report-v9-joint.json · margin_sweep</span>
  </figcaption>
  <div class="plot">
    <svg viewBox="0 0 900 300" role="img" aria-label="The prediction mismatch rate falling from 46.4 percent at a margin of zero to 30.6 percent at a margin of 200, as the encoder takes the prediction more often">
    <line x1="70.0" y1="236.0" x2="860.0" y2="236.0" stroke="var(--grid)" stroke-width="1.0" />
    <text x="60.0" y="240.0" font-size="11" fill="var(--ink-faint)" text-anchor="end" font-weight="400">31%</text>
    <line x1="70.0" y1="172.7" x2="860.0" y2="172.7" stroke="var(--grid)" stroke-width="1.0" />
    <text x="60.0" y="176.7" font-size="11" fill="var(--ink-faint)" text-anchor="end" font-weight="400">36%</text>
    <line x1="70.0" y1="109.3" x2="860.0" y2="109.3" stroke="var(--grid)" stroke-width="1.0" />
    <text x="60.0" y="113.3" font-size="11" fill="var(--ink-faint)" text-anchor="end" font-weight="400">41%</text>
    <line x1="70.0" y1="46.0" x2="860.0" y2="46.0" stroke="var(--grid)" stroke-width="1.0" />
    <text x="60.0" y="50.0" font-size="11" fill="var(--ink-faint)" text-anchor="end" font-weight="400">46%</text>
    <polyline points="70.0,46.0 267.5,67.5 465.0,92.2 662.5,147.6 860.0,236.0" fill="none" stroke="var(--sev-warning)" stroke-width="2.2" />
    <circle cx="70.0" cy="46.0" r="4.5" fill="var(--sev-warning)" />
    <text x="70.0" y="34.0" font-size="11" fill="var(--ink)" text-anchor="middle" font-weight="500">46.4%</text>
    <text x="70.0" y="256.0" font-size="11" fill="var(--ink-faint)" text-anchor="middle" font-weight="400">0</text>
    <circle cx="267.5" cy="67.5" r="4.5" fill="var(--sev-warning)" />
    <text x="267.5" y="55.5" font-size="11" fill="var(--ink)" text-anchor="middle" font-weight="500">44.6%</text>
    <text x="267.5" y="256.0" font-size="11" fill="var(--ink-faint)" text-anchor="middle" font-weight="400">25</text>
    <circle cx="465.0" cy="92.2" r="4.5" fill="var(--sev-warning)" />
    <text x="465.0" y="80.2" font-size="11" fill="var(--ink)" text-anchor="middle" font-weight="500">42.6%</text>
    <text x="465.0" y="256.0" font-size="11" fill="var(--ink-faint)" text-anchor="middle" font-weight="400">50</text>
    <circle cx="662.5" cy="147.6" r="4.5" fill="var(--sev-warning)" />
    <text x="662.5" y="135.6" font-size="11" fill="var(--ink)" text-anchor="middle" font-weight="500">37.9%</text>
    <text x="662.5" y="256.0" font-size="11" fill="var(--ink-faint)" text-anchor="middle" font-weight="400">100</text>
    <circle cx="860.0" cy="236.0" r="4.5" fill="var(--sev-warning)" />
    <text x="860.0" y="224.0" font-size="11" fill="var(--ink)" text-anchor="middle" font-weight="500">30.6%</text>
    <text x="860.0" y="256.0" font-size="11" fill="var(--ink-faint)" text-anchor="middle" font-weight="400">200</text>
    <text x="70.0" y="24.0" font-size="12" fill="var(--ink-dim)" text-anchor="start" font-weight="400">the margin is what turns a wrong prediction into a free one</text>
    <text x="465.0" y="280.0" font-size="11.5" fill="var(--ink-dim)" text-anchor="middle" font-weight="400">margin, in rate-distortion cost units</text>
    <text x="860.0" y="24.0" font-size="11.5" fill="var(--ink-faint)" text-anchor="end" font-weight="400">the intra blocks' mismatch is higher than the inter blocks'</text>
    </svg>
  </div>
</figure>
## Per sequence at the optimum

The two sequences where the mechanism loses are the two with the least motion: a flat, high-detail picture gives the predictor nothing directional to work with.

| Sequence | BD-rate | Content |
|---|---|---|
| deadline_cif | **−0.835** | Motion, texture and noise |
| foreman_cif | **−0.519** | Fine texture, strong edges |
| stefan_cif | **−0.645** | Motion, texture and noise |
| coastguard_cif | **−0.452** | Water texture, fine detail |
| container_cif | **−0.088** | Sharp edges, flat panels |
| bus_cif | **−0.052** | Moving subject, uniform background |
| flower_cif | **−0.248** | Fine foliage texture |
| city_cif | **−0.326** | Dense detail, little large-scale structure |
| mobile_cif | **−0.271** | Moving subject, detailed background |
| crew_cif | **−0.417** | Motion and texture |
| highway_cif | **−0.318** | Fast panning, strong horizontal structure |
| akiyo_cif | +1.341 | Flat, high detail, little motion |

The mean over the twelve is −0.236% and the median −0.322%.

<figure class="chart">
  <figcaption>
    <span>The learned predictor against the constant one, per sequence</span>
    <span class="src">research/benchmark/report-v9-joint.json · mechanism_bd_rate, constant_bd_rate</span>
  </figcaption>
  <div class="plot">
    <svg viewBox="0 0 900 380" role="img" aria-label="Per-sequence Bjontegaard delta rate for the shipped learned model and for the constant predictor that always answers the plain DCT-II">
    <line x1="70.0" y1="294.0" x2="860.0" y2="294.0" stroke="var(--grid)" stroke-width="1.0" />
    <text x="60.0" y="298.0" font-size="11" fill="var(--ink-faint)" text-anchor="end" font-weight="400">-1.2</text>
    <line x1="70.0" y1="210.7" x2="860.0" y2="210.7" stroke="var(--grid)" stroke-width="1.0" />
    <text x="60.0" y="214.7" font-size="11" fill="var(--ink-faint)" text-anchor="end" font-weight="400">-0.1</text>
    <line x1="70.0" y1="127.3" x2="860.0" y2="127.3" stroke="var(--grid)" stroke-width="1.0" />
    <text x="60.0" y="131.3" font-size="11" fill="var(--ink-faint)" text-anchor="end" font-weight="400">1.0</text>
    <line x1="70.0" y1="44.0" x2="860.0" y2="44.0" stroke="var(--grid)" stroke-width="1.0" />
    <text x="60.0" y="48.0" font-size="11" fill="var(--ink-faint)" text-anchor="end" font-weight="400">2.1</text>
    <line x1="70.0" y1="201.1" x2="860.0" y2="201.1" stroke="var(--line-strong)" stroke-width="1.4" />
    <text x="860.0" y="193.1" font-size="11.5" fill="var(--ink-dim)" text-anchor="end" font-weight="400">no gain</text>
    <rect x="47.9" y="101.0" width="21.1" height="100.1" fill="var(--accent)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <text x="58.5" y="213.1" font-size="9.5" fill="var(--ink-dim)" text-anchor="middle" font-weight="400">+1.34</text>
    <rect x="71.0" y="72.8" width="21.1" height="128.2" fill="var(--sev-info)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <text x="81.5" y="213.1" font-size="9.5" fill="var(--ink-dim)" text-anchor="middle" font-weight="400">+1.72</text>
    <text x="102.9" y="314.0" font-size="10.5" fill="var(--ink-faint)" text-anchor="middle" font-weight="400">akiyo</text>
    <rect x="113.8" y="201.1" width="21.1" height="3.9" fill="var(--accent)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <text x="124.3" y="196.1" font-size="9.5" fill="var(--ink-dim)" text-anchor="middle" font-weight="400">-0.05</text>
    <rect x="136.8" y="201.1" width="21.1" height="21.9" fill="var(--sev-info)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <text x="147.4" y="196.1" font-size="9.5" fill="var(--ink-dim)" text-anchor="middle" font-weight="400">-0.29</text>
    <text x="168.7" y="314.0" font-size="10.5" fill="var(--ink-faint)" text-anchor="middle" font-weight="400">bus</text>
    <rect x="179.6" y="201.1" width="21.1" height="24.3" fill="var(--accent)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <text x="190.1" y="196.1" font-size="9.5" fill="var(--ink-dim)" text-anchor="middle" font-weight="400">-0.33</text>
    <rect x="202.7" y="201.1" width="21.1" height="35.8" fill="var(--sev-info)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <text x="213.2" y="196.1" font-size="9.5" fill="var(--ink-dim)" text-anchor="middle" font-weight="400">-0.48</text>
    <text x="234.6" y="314.0" font-size="10.5" fill="var(--ink-faint)" text-anchor="middle" font-weight="400">city</text>
    <rect x="245.4" y="201.1" width="21.1" height="33.7" fill="var(--accent)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <text x="256.0" y="196.1" font-size="9.5" fill="var(--ink-dim)" text-anchor="middle" font-weight="400">-0.45</text>
    <rect x="268.5" y="199.4" width="21.1" height="1.7" fill="var(--sev-info)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <text x="279.0" y="213.1" font-size="9.5" fill="var(--ink-dim)" text-anchor="middle" font-weight="400">+0.02</text>
    <text x="300.4" y="314.0" font-size="10.5" fill="var(--ink-faint)" text-anchor="middle" font-weight="400">coastguard</text>
    <rect x="311.3" y="201.1" width="21.1" height="6.6" fill="var(--accent)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <text x="321.8" y="196.1" font-size="9.5" fill="var(--ink-dim)" text-anchor="middle" font-weight="400">-0.09</text>
    <rect x="334.3" y="201.1" width="21.1" height="9.7" fill="var(--sev-info)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <text x="344.9" y="196.1" font-size="9.5" fill="var(--ink-dim)" text-anchor="middle" font-weight="400">-0.13</text>
    <text x="366.2" y="314.0" font-size="10.5" fill="var(--ink-faint)" text-anchor="middle" font-weight="400">container</text>
    <rect x="377.1" y="201.1" width="21.1" height="31.1" fill="var(--accent)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <text x="387.6" y="196.1" font-size="9.5" fill="var(--ink-dim)" text-anchor="middle" font-weight="400">-0.42</text>
    <rect x="400.2" y="201.1" width="21.1" height="4.7" fill="var(--sev-info)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <text x="410.7" y="196.1" font-size="9.5" fill="var(--ink-dim)" text-anchor="middle" font-weight="400">-0.06</text>
    <text x="432.1" y="314.0" font-size="10.5" fill="var(--ink-faint)" text-anchor="middle" font-weight="400">crew</text>
    <rect x="442.9" y="201.1" width="21.1" height="62.3" fill="var(--accent)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <text x="453.5" y="196.1" font-size="9.5" fill="var(--ink-dim)" text-anchor="middle" font-weight="400">-0.83</text>
    <rect x="466.0" y="201.1" width="21.1" height="64.1" fill="var(--sev-info)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <text x="476.5" y="196.1" font-size="9.5" fill="var(--ink-dim)" text-anchor="middle" font-weight="400">-0.86</text>
    <text x="497.9" y="314.0" font-size="10.5" fill="var(--ink-faint)" text-anchor="middle" font-weight="400">deadline</text>
    <rect x="508.8" y="201.1" width="21.1" height="18.5" fill="var(--accent)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <text x="519.3" y="196.1" font-size="9.5" fill="var(--ink-dim)" text-anchor="middle" font-weight="400">-0.25</text>
    <rect x="531.8" y="201.1" width="21.1" height="30.0" fill="var(--sev-info)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <text x="542.4" y="196.1" font-size="9.5" fill="var(--ink-dim)" text-anchor="middle" font-weight="400">-0.40</text>
    <text x="563.7" y="314.0" font-size="10.5" fill="var(--ink-faint)" text-anchor="middle" font-weight="400">flower</text>
    <rect x="574.6" y="201.1" width="21.1" height="38.7" fill="var(--accent)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <text x="585.1" y="196.1" font-size="9.5" fill="var(--ink-dim)" text-anchor="middle" font-weight="400">-0.52</text>
    <rect x="597.7" y="201.1" width="21.1" height="20.8" fill="var(--sev-info)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <text x="608.2" y="196.1" font-size="9.5" fill="var(--ink-dim)" text-anchor="middle" font-weight="400">-0.28</text>
    <text x="629.6" y="314.0" font-size="10.5" fill="var(--ink-faint)" text-anchor="middle" font-weight="400">foreman</text>
    <rect x="640.4" y="201.1" width="21.1" height="23.8" fill="var(--accent)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <text x="651.0" y="196.1" font-size="9.5" fill="var(--ink-dim)" text-anchor="middle" font-weight="400">-0.32</text>
    <rect x="663.5" y="201.1" width="21.1" height="27.4" fill="var(--sev-info)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <text x="674.0" y="196.1" font-size="9.5" fill="var(--ink-dim)" text-anchor="middle" font-weight="400">-0.37</text>
    <text x="695.4" y="314.0" font-size="10.5" fill="var(--ink-faint)" text-anchor="middle" font-weight="400">highway</text>
    <rect x="706.3" y="201.1" width="21.1" height="20.2" fill="var(--accent)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <text x="716.8" y="196.1" font-size="9.5" fill="var(--ink-dim)" text-anchor="middle" font-weight="400">-0.27</text>
    <rect x="729.3" y="201.1" width="21.1" height="16.5" fill="var(--sev-info)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <text x="739.9" y="196.1" font-size="9.5" fill="var(--ink-dim)" text-anchor="middle" font-weight="400">-0.22</text>
    <text x="761.2" y="314.0" font-size="10.5" fill="var(--ink-faint)" text-anchor="middle" font-weight="400">mobile</text>
    <rect x="772.1" y="201.1" width="21.1" height="48.1" fill="var(--accent)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <text x="782.6" y="196.1" font-size="9.5" fill="var(--ink-dim)" text-anchor="middle" font-weight="400">-0.65</text>
    <rect x="795.2" y="201.1" width="21.1" height="6.3" fill="var(--sev-info)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <text x="805.7" y="196.1" font-size="9.5" fill="var(--ink-dim)" text-anchor="middle" font-weight="400">-0.08</text>
    <text x="827.1" y="314.0" font-size="10.5" fill="var(--ink-faint)" text-anchor="middle" font-weight="400">stefan</text>
    <rect x="70.0" y="20.0" width="14.0" height="10.0" fill="var(--accent)" stroke="var(--line-strong)" stroke-width="0.5" />
    <text x="90.0" y="29.0" font-size="12" fill="var(--ink-dim)" text-anchor="start" font-weight="400">the shipped learned model</text>
    <rect x="330.0" y="20.0" width="14.0" height="10.0" fill="var(--sev-info)" stroke="var(--line-strong)" stroke-width="0.5" />
    <text x="350.0" y="29.0" font-size="12" fill="var(--ink-dim)" text-anchor="start" font-weight="400">the constant predictor, always the plain DCT-II</text>
    </svg>
  </div>
</figure>
## How predictable is the transform?

The predictor's accuracy is the mechanism's ceiling, so the training pipeline measures it two ways: the shipped logistic regression, and a gradient-boosted tree fitted over the same features as an upper bound on what any model over them can achieve.

| Measure | Accuracy | Reading |
|---|---|---|
| Majority class, always the plain DCT-II | 50.10% | The baseline any predictor must beat |
| Gradient-boosted ceiling, joint mode | 50.65% | What these features support for the mode as a whole |
| Shipped model, over the joint mode | 50.17% | 91.6% on the plain DCT-II, 5.4% to 15.2% on the four others |

570,240 blocks, collected from the encoder's own rate-distortion search over the twelve sequences, so the labels are the encoder's real choices and the features are what the decoder will actually see.

<figure class="chart">
  <figcaption>
    <span>The predictability ceiling, and where the shipped model sits under it</span>
    <span class="src">models/girc-predictor.json · training.predictability_ceiling</span>
  </figcaption>
  <div class="plot">
    <svg viewBox="0 0 900 300" role="img" aria-label="The majority class at 50.10 percent, the shipped model at 50.17 percent, and the gradient-boosted ceiling at 50.65 percent">
    <rect x="70.0" y="46.0" width="7.2" height="30.0" fill="var(--ink-faint)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <text x="87.2" y="66.0" font-size="12" fill="var(--ink)" text-anchor="start" font-weight="500">50.10%</text>
    <text x="70.0" y="38.0" font-size="12" fill="var(--ink-dim)" text-anchor="start" font-weight="400">majority class: always the plain DCT-II</text>
    <text x="77.2" y="92.0" font-size="10.5" fill="var(--ink-faint)" text-anchor="start" font-weight="400">50.10 of every 100 blocks</text>
    <rect x="70.0" y="102.0" width="7.2" height="30.0" fill="var(--accent)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <text x="87.2" y="122.0" font-size="12" fill="var(--ink)" text-anchor="start" font-weight="500">50.17%</text>
    <text x="70.0" y="94.0" font-size="12" fill="var(--ink-dim)" text-anchor="start" font-weight="400">the shipped model, over the joint mode</text>
    <text x="77.2" y="148.0" font-size="10.5" fill="var(--ink-faint)" text-anchor="start" font-weight="400">50.17 of every 100 blocks</text>
    <rect x="70.0" y="158.0" width="7.3" height="30.0" fill="var(--sev-info)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <text x="87.3" y="178.0" font-size="12" fill="var(--ink)" text-anchor="start" font-weight="500">50.65%</text>
    <text x="70.0" y="150.0" font-size="12" fill="var(--ink-dim)" text-anchor="start" font-weight="400">the gradient-boosted ceiling, same features</text>
    <text x="77.3" y="204.0" font-size="10.5" fill="var(--ink-faint)" text-anchor="start" font-weight="400">50.65 of every 100 blocks</text>
    <text x="70.0" y="220.0" font-size="12" fill="var(--ink-faint)" text-anchor="start" font-weight="400">the gap between the first and the last is what any predictor over these features can earn</text>
    </svg>
  </div>
</figure>

<figure class="chart">
  <figcaption>
    <span>Per-class accuracy: the default is learned, the other four modes are not</span>
    <span class="src">models/girc-predictor.json · training.per_class_test_accuracy</span>
  </figcaption>
  <div class="plot">
    <svg viewBox="0 0 900 300" role="img" aria-label="Per-class test accuracy: 91.7 percent on the plain DCT-II and between 5.4 and 15.2 percent on the four multiple transform selection modes">
    <rect x="28.0" y="50.0" width="146.8" height="190.0" fill="var(--sev-ok)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <text x="28.0" y="42.0" font-size="11.5" fill="var(--ink)" text-anchor="start" font-weight="500">91.7%</text>
    <text x="28.0" y="258.0" font-size="10.5" fill="var(--ink-faint)" text-anchor="start" font-weight="400">DCT-II</text>
    <rect x="200.8" y="208.5" width="146.8" height="31.5" fill="var(--sev-critical)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <text x="200.8" y="200.5" font-size="11.5" fill="var(--ink)" text-anchor="start" font-weight="500">15.2%</text>
    <text x="200.8" y="258.0" font-size="10.5" fill="var(--ink-faint)" text-anchor="start" font-weight="400">DST-VII</text>
    <text x="200.8" y="273.0" font-size="10.5" fill="var(--ink-faint)" text-anchor="start" font-weight="400">DST-VII</text>
    <rect x="373.6" y="228.9" width="146.8" height="11.1" fill="var(--sev-critical)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <text x="373.6" y="220.9" font-size="11.5" fill="var(--ink)" text-anchor="start" font-weight="500">5.4%</text>
    <text x="373.6" y="258.0" font-size="10.5" fill="var(--ink-faint)" text-anchor="start" font-weight="400">DCT-VIII</text>
    <text x="373.6" y="273.0" font-size="10.5" fill="var(--ink-faint)" text-anchor="start" font-weight="400">DST-VII</text>
    <rect x="546.4" y="228.2" width="146.8" height="11.8" fill="var(--sev-critical)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <text x="546.4" y="220.2" font-size="11.5" fill="var(--ink)" text-anchor="start" font-weight="500">5.7%</text>
    <text x="546.4" y="258.0" font-size="10.5" fill="var(--ink-faint)" text-anchor="start" font-weight="400">DST-VII</text>
    <text x="546.4" y="273.0" font-size="10.5" fill="var(--ink-faint)" text-anchor="start" font-weight="400">DCT-VIII</text>
    <rect x="719.2" y="226.4" width="146.8" height="13.6" fill="var(--sev-critical)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <text x="719.2" y="218.4" font-size="11.5" fill="var(--ink)" text-anchor="start" font-weight="500">6.6%</text>
    <text x="719.2" y="258.0" font-size="10.5" fill="var(--ink-faint)" text-anchor="start" font-weight="400">DCT-VIII</text>
    <text x="719.2" y="273.0" font-size="10.5" fill="var(--ink-faint)" text-anchor="start" font-weight="400">DCT-VIII</text>
    <line x1="18.0" y1="240.0" x2="882.0" y2="240.0" stroke="var(--line-strong)" stroke-width="1.0" />
    <text x="18.0" y="24.0" font-size="12" fill="var(--ink-dim)" text-anchor="start" font-weight="400">the model answers the default correctly almost always, and the four directional modes almost never</text>
    <text x="882.0" y="24.0" font-size="12" fill="var(--ink-faint)" text-anchor="end" font-weight="400">114,048 held-out blocks</text>
    </svg>
  </div>
</figure>

The joint mode is barely predictable, and the reason is worth stating rather than hiding. An earlier revision reported 53.4% accuracy, and that figure was flattered by a feature built from the intra mode of an inter block: the decoder cannot know that mode, because the block carries a motion vector instead. The feature is empty for an inter block in the shipped code, and the benchmark decodes every stream it encodes and refuses a result if the decoded picture differs from the encoder's own reconstruction.

## Intra and inter

Inter prediction is implemented and measured. The inter blocks' mismatch rate is consistently lower than the intra blocks', so the hypothesis holds, but the gain over the majority baseline is smaller for the inter blocks than for the intra ones, because the inter blocks' transform distribution is more concentrated: there is less to gain when one mode already dominates.

| Block kind | Records | Horizontal gain | Vertical gain |
|---|---|---|---|
| intra | 316,808 | +7.50 points | +7.40 points |
| inter | 272,440 | +1.89 points | +2.05 points |

<figure class="chart">
  <figcaption>
    <span>Intra against inter: the hypothesis holds, and the gain is smaller for inter</span>
    <span class="src">research/benchmark/inter-predictability.json</span>
  </figcaption>
  <div class="plot">
    <svg viewBox="0 0 900 280" role="img" aria-label="Per-axis prediction accuracy over the majority baseline: a gain of 7.5 percentage points for the intra blocks and 1.9 for the inter blocks">
    <rect x="300.0" y="50.0" width="219.1" height="30.0" fill="var(--ink-faint)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.5" />
    <rect x="300.0" y="50.0" width="259.6" height="30.0" fill="var(--accent)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <text x="288.0" y="70.0" font-size="11.5" fill="var(--ink-dim)" text-anchor="end" font-weight="400">intra, horizontal axis</text>
    <text x="569.6" y="70.0" font-size="12" fill="var(--ink)" text-anchor="start" font-weight="500">48.08%</text>
    <text x="519.1" y="94.0" font-size="10.5" fill="var(--ink-faint)" text-anchor="start" font-weight="400">majority 40.58% · a gain of +7.50 points over 316,808 records</text>
    <rect x="300.0" y="104.0" width="219.1" height="30.0" fill="var(--ink-faint)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.5" />
    <rect x="300.0" y="104.0" width="259.1" height="30.0" fill="var(--accent)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <text x="288.0" y="124.0" font-size="11.5" fill="var(--ink-dim)" text-anchor="end" font-weight="400">intra, vertical axis</text>
    <text x="569.1" y="124.0" font-size="12" fill="var(--ink)" text-anchor="start" font-weight="500">47.98%</text>
    <text x="519.1" y="148.0" font-size="10.5" fill="var(--ink-faint)" text-anchor="start" font-weight="400">majority 40.58% · a gain of +7.40 points over 316,808 records</text>
    <rect x="300.0" y="158.0" width="326.7" height="30.0" fill="var(--ink-faint)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.5" />
    <rect x="300.0" y="158.0" width="336.9" height="30.0" fill="var(--sev-info)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <text x="288.0" y="178.0" font-size="11.5" fill="var(--ink-dim)" text-anchor="end" font-weight="400">inter, horizontal axis</text>
    <text x="646.9" y="178.0" font-size="12" fill="var(--ink)" text-anchor="start" font-weight="500">62.39%</text>
    <text x="626.7" y="202.0" font-size="10.5" fill="var(--ink-faint)" text-anchor="start" font-weight="400">majority 60.50% · a gain of +1.89 points over 272,440 records</text>
    <rect x="300.0" y="212.0" width="326.7" height="30.0" fill="var(--ink-faint)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.5" />
    <rect x="300.0" y="212.0" width="337.8" height="30.0" fill="var(--sev-info)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <text x="288.0" y="232.0" font-size="11.5" fill="var(--ink-dim)" text-anchor="end" font-weight="400">inter, vertical axis</text>
    <text x="647.8" y="232.0" font-size="12" fill="var(--ink)" text-anchor="start" font-weight="500">62.55%</text>
    <text x="626.7" y="256.0" font-size="10.5" fill="var(--ink-faint)" text-anchor="start" font-weight="400">majority 60.50% · a gain of +2.05 points over 272,440 records</text>
    <text x="40.0" y="24.0" font-size="12" fill="var(--ink-dim)" text-anchor="start" font-weight="400">the inter blocks' transform distribution is more concentrated, so there is less to gain</text>
    <text x="40.0" y="262.0" font-size="11.5" fill="var(--ink-faint)" text-anchor="start" font-weight="400">measured on the encoder's own records, not on a model of them</text>
    </svg>
  </div>
</figure>

<figure class="chart">
  <figcaption>
    <span>What the encoder actually chose: the plain DCT-II, and four modes that split the rest</span>
    <span class="src">models/girc-predictor.json · class_distribution</span>
  </figcaption>
  <div class="plot">
    <svg viewBox="0 0 900 300" role="img" aria-label="The transform mode distribution over 570,240 blocks: the plain DCT-II at 50.1 percent, and the four multiple transform selection modes sharing 49.9 percent">
    <line x1="18.0" y1="256.0" x2="882.0" y2="256.0" stroke="var(--line-strong)" stroke-width="1.0" />
    <line x1="18.0" y1="56.0" x2="18.0" y2="256.0" stroke="var(--line-strong)" stroke-width="1.0" />
    <rect x="28.0" y="90.0" width="150.8" height="166.0" fill="var(--accent)" stroke="var(--line-strong)" stroke-width="0.5" />
    <text x="28.0" y="82.0" font-size="12" fill="var(--ink)" text-anchor="start" font-weight="500">50.1%</text>
    <text x="28.0" y="274.0" font-size="11" fill="var(--ink-faint)" text-anchor="start" font-weight="400">DCT-II</text>
    <rect x="200.8" y="208.9" width="150.8" height="47.1" fill="var(--sev-info)" stroke="var(--line-strong)" stroke-width="0.5" />
    <text x="200.8" y="200.9" font-size="12" fill="var(--ink)" text-anchor="start" font-weight="500">14.2%</text>
    <text x="200.8" y="274.0" font-size="11" fill="var(--ink-faint)" text-anchor="start" font-weight="400">DST-VII</text>
    <text x="200.8" y="289.0" font-size="11" fill="var(--ink-faint)" text-anchor="start" font-weight="400">DST-VII</text>
    <rect x="373.6" y="215.1" width="150.8" height="40.9" fill="var(--sev-info)" stroke="var(--line-strong)" stroke-width="0.5" />
    <text x="373.6" y="207.1" font-size="12" fill="var(--ink)" text-anchor="start" font-weight="500">12.3%</text>
    <text x="373.6" y="274.0" font-size="11" fill="var(--ink-faint)" text-anchor="start" font-weight="400">DCT-VIII</text>
    <text x="373.6" y="289.0" font-size="11" fill="var(--ink-faint)" text-anchor="start" font-weight="400">DST-VII</text>
    <rect x="546.4" y="214.8" width="150.8" height="41.2" fill="var(--sev-info)" stroke="var(--line-strong)" stroke-width="0.5" />
    <text x="546.4" y="206.8" font-size="12" fill="var(--ink)" text-anchor="start" font-weight="500">12.4%</text>
    <text x="546.4" y="274.0" font-size="11" fill="var(--ink-faint)" text-anchor="start" font-weight="400">DST-VII</text>
    <text x="546.4" y="289.0" font-size="11" fill="var(--ink-faint)" text-anchor="start" font-weight="400">DCT-VIII</text>
    <rect x="719.2" y="219.8" width="150.8" height="36.2" fill="var(--sev-info)" stroke="var(--line-strong)" stroke-width="0.5" />
    <text x="719.2" y="211.8" font-size="12" fill="var(--ink)" text-anchor="start" font-weight="500">10.9%</text>
    <text x="719.2" y="274.0" font-size="11" fill="var(--ink-faint)" text-anchor="start" font-weight="400">DCT-VIII</text>
    <text x="719.2" y="289.0" font-size="11" fill="var(--ink-faint)" text-anchor="start" font-weight="400">DCT-VIII</text>
    <text x="18.0" y="24.0" font-size="12.5" fill="var(--ink-dim)" text-anchor="start" font-weight="400">570,240 blocks, the encoder's own rate-distortion choices</text>
    <text x="882.0" y="24.0" font-size="12" fill="var(--ink-faint)" text-anchor="end" font-weight="400">the majority class is 50.10%</text>
    <text x="882.0" y="44.0" font-size="12" fill="var(--ink-faint)" text-anchor="end" font-weight="400">the four other modes are 49.90%</text>
    </svg>
  </div>
</figure>
## Why the mechanism is small

The transform choice is driven by the block's own residual, which the decoder has not seen, and the geometry of the neighbourhood constrains it only weakly. The rate-distortion choice between the modes is close to a tie, so there is little for a predictor to predict. That is measured rather than asserted: the gap between the search's best transform and the runner-up has a median of 0.098 rate-distortion cost units and a 90th percentile of 0.695.

<figure class="chart">
  <figcaption>
    <span>Why the mechanism is small: the rate-distortion choice between modes is close to a tie</span>
    <span class="src">research/benchmark/transform-spread.json</span>
  </figcaption>
  <div class="plot">
    <svg viewBox="0 0 900 260" role="img" aria-label="The gap between the search's best transform and the runner-up, as a percentage of the best: a median of 0.67 percent and a 90th percentile of 2.64 percent">
    <text x="14.0" y="30.0" font-size="11.5" fill="var(--ink-dim)" text-anchor="start" font-weight="400">RD-cost gap</text>
    <rect x="70.0" y="46.0" width="0.0" height="28.0" fill="var(--ink-faint)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <text x="80.0" y="65.0" font-size="12" fill="var(--ink)" text-anchor="start" font-weight="500">0.000</text>
    <text x="70.0" y="38.0" font-size="11.5" fill="var(--ink-dim)" text-anchor="start" font-weight="400">10th percentile</text>
    <rect x="70.0" y="94.0" width="21.2" height="28.0" fill="var(--accent)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <text x="101.2" y="113.0" font-size="12" fill="var(--ink)" text-anchor="start" font-weight="500">0.098</text>
    <text x="70.0" y="86.0" font-size="11.5" fill="var(--ink-dim)" text-anchor="start" font-weight="400">median</text>
    <rect x="70.0" y="142.0" width="150.0" height="28.0" fill="var(--sev-warning)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <text x="230.0" y="161.0" font-size="12" fill="var(--ink)" text-anchor="start" font-weight="500">0.695</text>
    <text x="70.0" y="134.0" font-size="11.5" fill="var(--ink-dim)" text-anchor="start" font-weight="400">90th percentile</text>
    <text x="70.0" y="196.0" font-size="11.5" fill="var(--ink-faint)" text-anchor="start" font-weight="400">relative spread: a median of 0.67 and a 90th percentile of 2.64 percent of the best mode's own cost, over 1,584 blocks of akiyo_cif</text>
    </svg>
  </div>
</figure>

The mechanism is therefore a signalling-removal technique whose value grows with the number of transform choices and with how evenly they are distributed. A codec with a richer residual coder, where the modes separate more, is where it should pay.

## The signalling cost

What the two syntaxes cost per block, against each other, measured from the benchmark's own block counts and the model's own class distribution:

<figure class="chart">
  <figcaption>
    <span>What the signalling costs per block, against the explicit syntax it replaces</span>
    <span class="src">research/benchmark/report-v9-joint.json · results, models/girc-predictor.json</span>
  </figcaption>
  <div class="plot">
    <svg viewBox="0 0 900 300" role="img" aria-label="Signalling bins per block: the explicit syntax costs 2.12 bins, the constant predictor 1.99, the learned model 1.97, and the oracle one bin">
    <rect x="330.0" y="52.0" width="520.0" height="30.0" fill="var(--ink-dim)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <text x="860.0" y="72.0" font-size="12" fill="var(--ink)" text-anchor="start" font-weight="500">2.09 bins</text>
    <text x="330.0" y="44.0" font-size="11.5" fill="var(--ink-dim)" text-anchor="start" font-weight="400">the explicit mode, the standard's own mts_idx syntax</text>
    <text x="330.0" y="96.0" font-size="10.5" fill="var(--ink-faint)" text-anchor="start" font-weight="400">the baseline</text>
    <rect x="330.0" y="108.0" width="498.3" height="30.0" fill="var(--sev-info)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <text x="838.3" y="128.0" font-size="12" fill="var(--ink)" text-anchor="start" font-weight="500">2.00 bins</text>
    <text x="330.0" y="100.0" font-size="11.5" fill="var(--ink-dim)" text-anchor="start" font-weight="400">the constant predictor, always the plain DCT-II</text>
    <text x="330.0" y="152.0" font-size="10.5" fill="var(--ink-faint)" text-anchor="start" font-weight="400">no training, no model</text>
    <rect x="330.0" y="164.0" width="460.8" height="30.0" fill="var(--accent)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <text x="800.8" y="184.0" font-size="12" fill="var(--ink)" text-anchor="start" font-weight="500">1.85 bins</text>
    <text x="330.0" y="156.0" font-size="11.5" fill="var(--ink-dim)" text-anchor="start" font-weight="400">the shipped learned model</text>
    <text x="330.0" y="208.0" font-size="10.5" fill="var(--ink-faint)" text-anchor="start" font-weight="400">the mechanism as shipped</text>
    <rect x="330.0" y="220.0" width="248.9" height="30.0" fill="var(--sev-critical)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <text x="588.9" y="240.0" font-size="12" fill="var(--ink)" text-anchor="start" font-weight="500">1.00 bins</text>
    <text x="330.0" y="212.0" font-size="11.5" fill="var(--ink-dim)" text-anchor="start" font-weight="400">the oracle, which cannot be built</text>
    <text x="330.0" y="264.0" font-size="10.5" fill="var(--ink-faint)" text-anchor="start" font-weight="400">the ceiling</text>
    <text x="18.0" y="24.0" font-size="12" fill="var(--ink-dim)" text-anchor="start" font-weight="400">the correction is coded relative to the prediction, so a wrong answer costs three bins rather than two</text>
    <text x="880.0" y="24.0" font-size="11" fill="var(--ink-faint)" text-anchor="end" font-weight="400">760,320 blocks, the benchmark's own count</text>
    </svg>
  </div>
</figure>

## The mechanism

VVC already has implicit multiple transform selection: it derives the transform type from already-coded information, the block size and the neighbour availability, and spends zero bits on it. GIRC generalises that. Instead of deriving the transform from a fixed table indexed by syntax, it predicts it from the geometry the decoder has reconstructed, and when the prediction is wrong it codes a correction relative to the prediction rather than the type itself.

<figure class="chart">
  <figcaption>
    <span>How the prediction works: the window, the block, and the single correction bit</span>
    <span class="src">crates/girc-core/src/geometry.rs, crates/girc-core/src/bitstream.rs</span>
  </figcaption>
  <div class="plot">
    <svg viewBox="0 0 900 300" role="img" aria-label="A diagram of the causal window the decoder reads, the block whose transform is predicted, and the one bin the encoder spends when the prediction was wrong">
    <rect x="240" y="60" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="254" y="60" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="268" y="60" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="282" y="60" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="296" y="60" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="310" y="60" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="324" y="60" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="338" y="60" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="352" y="60" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="366" y="60" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="380" y="60" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="394" y="60" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="240" y="74" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="254" y="74" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="268" y="74" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="282" y="74" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="296" y="74" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="310" y="74" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="324" y="74" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="338" y="74" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="352" y="74" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="366" y="74" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="380" y="74" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="394" y="74" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="240" y="88" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="254" y="88" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="268" y="88" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="282" y="88" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="296" y="88" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="310" y="88" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="324" y="88" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="338" y="88" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="352" y="88" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="366" y="88" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="380" y="88" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="394" y="88" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="240" y="102" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="254" y="102" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="268" y="102" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="282" y="102" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="296" y="102" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="310" y="102" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="324" y="102" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="338" y="102" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="352" y="102" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="366" y="102" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="380" y="102" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="394" y="102" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="240" y="116" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="254" y="116" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="268" y="116" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="282" y="116" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="296" y="116" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="310" y="116" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="324" y="116" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="338" y="116" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="352" y="116" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="366" y="116" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="380" y="116" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="394" y="116" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="240" y="130" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="254" y="130" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="268" y="130" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="282" y="130" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="296" y="130" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="310" y="130" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="324" y="130" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="338" y="130" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="352" y="130" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="366" y="130" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="380" y="130" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="394" y="130" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="240" y="144" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="254" y="144" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="268" y="144" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="282" y="144" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="296" y="144" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="310" y="144" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="324" y="144" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="338" y="144" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="352" y="144" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="366" y="144" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="380" y="144" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="394" y="144" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="240" y="158" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="254" y="158" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="268" y="158" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="282" y="158" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="296" y="158" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="310" y="158" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="324" y="158" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="338" y="158" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="352" y="158" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="366" y="158" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="380" y="158" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="394" y="158" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="240" y="172" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="254" y="172" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="268" y="172" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="282" y="172" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="296" y="172" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="310" y="172" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="324" y="172" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="338" y="172" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="352" y="172" width="14" height="14" fill="var(--panel)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="366" y="172" width="14" height="14" fill="var(--panel)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="380" y="172" width="14" height="14" fill="var(--panel)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="394" y="172" width="14" height="14" fill="var(--panel)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="240" y="186" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="254" y="186" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="268" y="186" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="282" y="186" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="296" y="186" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="310" y="186" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="324" y="186" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="338" y="186" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="352" y="186" width="14" height="14" fill="var(--panel)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="366" y="186" width="14" height="14" fill="var(--panel)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="380" y="186" width="14" height="14" fill="var(--panel)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="394" y="186" width="14" height="14" fill="var(--panel)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="240" y="200" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="254" y="200" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="268" y="200" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="282" y="200" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="296" y="200" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="310" y="200" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="324" y="200" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="338" y="200" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="352" y="200" width="14" height="14" fill="var(--panel)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="366" y="200" width="14" height="14" fill="var(--panel)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="380" y="200" width="14" height="14" fill="var(--panel)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="394" y="200" width="14" height="14" fill="var(--panel)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="240" y="214" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="254" y="214" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="268" y="214" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="282" y="214" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="296" y="214" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="310" y="214" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="324" y="214" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="338" y="214" width="14" height="14" fill="var(--bar-track)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="352" y="214" width="14" height="14" fill="var(--panel)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="366" y="214" width="14" height="14" fill="var(--panel)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="380" y="214" width="14" height="14" fill="var(--panel)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="394" y="214" width="14" height="14" fill="var(--panel)" stroke="var(--grid)" stroke-width="0.5" />
    <rect x="352" y="172" width="56" height="56" fill="none" stroke="var(--accent)" stroke-width="1.6" />
    <text x="354.0" y="162.0" font-size="12" fill="var(--accent)" text-anchor="start" font-weight="400">the block being coded</text>
    <text x="242.0" y="50.0" font-size="11.5" fill="var(--ink-dim)" text-anchor="start" font-weight="400">already reconstructed, already known to both sides</text>
    <rect x="40" y="40" width="280" height="46" rx="2" fill="var(--panel)" stroke="var(--line-strong)" stroke-width="1" />
    <text x="54.0" y="68.0" font-size="12" fill="var(--ink)" text-anchor="start" font-weight="400">the encoder's rate-distortion search</text>
    <path d="M 180 86 V 114" stroke="var(--accent)" stroke-width="1.4" fill="none" marker-end="url(#arrow)" />
    <rect x="40" y="114" width="280" height="46" rx="2" fill="var(--panel)" stroke="var(--line-strong)" stroke-width="1" />
    <text x="54.0" y="142.0" font-size="12" fill="var(--ink)" text-anchor="start" font-weight="400">the decoder's prediction, from the window</text>
    <path d="M 180 160 V 188" stroke="var(--accent)" stroke-width="1.4" fill="none" marker-end="url(#arrow)" />
    <rect x="40" y="188" width="280" height="46" rx="2" fill="var(--panel)" stroke="var(--line-strong)" stroke-width="1" />
    <text x="54.0" y="216.0" font-size="12" fill="var(--ink)" text-anchor="start" font-weight="400">one bin: was the prediction right?</text>
    <path d="M 180 234 V 262" stroke="var(--accent)" stroke-width="1.4" fill="none" marker-end="url(#arrow)" />
    <rect x="40" y="262" width="280" height="46" rx="2" fill="var(--panel)" stroke="var(--line-strong)" stroke-width="1" />
    <text x="54.0" y="290.0" font-size="12" fill="var(--ink)" text-anchor="start" font-weight="400">if it was wrong, two bins pick between the rest</text>
    <rect x="560" y="40" width="300" height="46" rx="2" fill="var(--panel)" stroke="var(--line-strong)" stroke-width="1" />
    <text x="574.0" y="68.0" font-size="12" fill="var(--ink)" text-anchor="start" font-weight="400">the decoder reads its own window</text>
    <path d="M 710 86 V 114" stroke="var(--sev-info)" stroke-width="1.4" fill="none" marker-end="url(#arrow2)" />
    <rect x="560" y="114" width="300" height="46" rx="2" fill="var(--panel)" stroke="var(--line-strong)" stroke-width="1" />
    <text x="574.0" y="142.0" font-size="12" fill="var(--ink)" text-anchor="start" font-weight="400">it predicts the mode, at no cost</text>
    <path d="M 710 160 V 188" stroke="var(--sev-info)" stroke-width="1.4" fill="none" marker-end="url(#arrow2)" />
    <rect x="560" y="188" width="300" height="46" rx="2" fill="var(--panel)" stroke="var(--line-strong)" stroke-width="1" />
    <text x="574.0" y="216.0" font-size="12" fill="var(--ink)" text-anchor="start" font-weight="400">it reads the one bin</text>
    <path d="M 710 234 V 262" stroke="var(--sev-info)" stroke-width="1.4" fill="none" marker-end="url(#arrow2)" />
    <rect x="560" y="262" width="300" height="46" rx="2" fill="var(--panel)" stroke="var(--line-strong)" stroke-width="1" />
    <text x="574.0" y="290.0" font-size="12" fill="var(--ink)" text-anchor="start" font-weight="400">right: take the prediction. wrong: read two more</text>
    <line x1="440.0" y1="254.0" x2="440.0" y2="254.0" stroke="var(--line-strong)" stroke-width="1.0" />
    <text x="440.0" y="280.0" font-size="11.5" fill="var(--ink-faint)" text-anchor="middle" font-weight="400">one bitstream, one flag in the header</text>
    <defs><marker id="arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M 0 0 L 10 5 L 0 10 z" fill="var(--accent)" /></marker><marker id="arrow2" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M 0 0 L 10 5 L 0 10 z" fill="var(--sev-info)" /></marker></defs>
    </svg>
  </div>
</figure>

<figure class="chart">
  <figcaption>
    <span>The two syntaxes, and what each costs</span>
    <span class="src">crates/girc-core/src/bitstream.rs, ITU-T H.266 clause 7.3.9</span>
  </figcaption>
  <div class="plot">
    <svg viewBox="0 0 900 280" role="img" aria-label="The explicit mode's truncated binary code over the five modes, against the inferred mode's one bin plus a two-bin correction">
    <rect x="40" y="56" width="180" height="30" rx="2" fill="var(--panel)" stroke="var(--line-strong)" stroke-width="1" />
    <text x="52.0" y="76.0" font-size="11.5" fill="var(--ink)" text-anchor="start" font-weight="400">plain DCT-II</text>
    <rect x="250" y="56" width="28" height="30" fill="var(--ink-dim)" opacity="0.3" stroke="var(--line-strong)" stroke-width="0.8" />
    <text x="264.0" y="76.0" font-size="11" fill="var(--ink)" text-anchor="start" font-weight="400">1</text>
    <text x="398.0" y="76.0" font-size="12" fill="var(--ink)" text-anchor="start" font-weight="500">1 bin</text>
    <rect x="40" y="96" width="180" height="30" rx="2" fill="var(--panel)" stroke="var(--line-strong)" stroke-width="1" />
    <text x="52.0" y="116.0" font-size="11.5" fill="var(--ink)" text-anchor="start" font-weight="400">DST-VII / DST-VII</text>
    <rect x="250" y="96" width="28" height="30" fill="var(--sev-info)" opacity="0.3" stroke="var(--line-strong)" stroke-width="0.8" />
    <text x="264.0" y="116.0" font-size="11" fill="var(--ink)" text-anchor="start" font-weight="400">1</text>
    <rect x="284" y="96" width="28" height="30" fill="var(--sev-info)" opacity="0.3" stroke="var(--line-strong)" stroke-width="0.8" />
    <text x="298.0" y="116.0" font-size="11" fill="var(--ink)" text-anchor="start" font-weight="400">0</text>
    <text x="398.0" y="116.0" font-size="12" fill="var(--ink)" text-anchor="start" font-weight="500">2 bins</text>
    <rect x="40" y="136" width="180" height="30" rx="2" fill="var(--panel)" stroke="var(--line-strong)" stroke-width="1" />
    <text x="52.0" y="156.0" font-size="11.5" fill="var(--ink)" text-anchor="start" font-weight="400">DCT-VIII / DST-VII</text>
    <rect x="250" y="136" width="28" height="30" fill="var(--sev-info)" opacity="0.3" stroke="var(--line-strong)" stroke-width="0.8" />
    <text x="264.0" y="156.0" font-size="11" fill="var(--ink)" text-anchor="start" font-weight="400">1</text>
    <rect x="284" y="136" width="28" height="30" fill="var(--sev-info)" opacity="0.3" stroke="var(--line-strong)" stroke-width="0.8" />
    <text x="298.0" y="156.0" font-size="11" fill="var(--ink)" text-anchor="start" font-weight="400">0</text>
    <rect x="318" y="136" width="28" height="30" fill="var(--sev-info)" opacity="0.3" stroke="var(--line-strong)" stroke-width="0.8" />
    <text x="332.0" y="156.0" font-size="11" fill="var(--ink)" text-anchor="start" font-weight="400">0</text>
    <text x="398.0" y="156.0" font-size="12" fill="var(--ink)" text-anchor="start" font-weight="500">3 bins</text>
    <rect x="40" y="176" width="180" height="30" rx="2" fill="var(--panel)" stroke="var(--line-strong)" stroke-width="1" />
    <text x="52.0" y="196.0" font-size="11.5" fill="var(--ink)" text-anchor="start" font-weight="400">DST-VII / DCT-VIII</text>
    <rect x="250" y="176" width="28" height="30" fill="var(--sev-info)" opacity="0.3" stroke="var(--line-strong)" stroke-width="0.8" />
    <text x="264.0" y="196.0" font-size="11" fill="var(--ink)" text-anchor="start" font-weight="400">1</text>
    <rect x="284" y="176" width="28" height="30" fill="var(--sev-info)" opacity="0.3" stroke="var(--line-strong)" stroke-width="0.8" />
    <text x="298.0" y="196.0" font-size="11" fill="var(--ink)" text-anchor="start" font-weight="400">0</text>
    <rect x="318" y="176" width="28" height="30" fill="var(--sev-info)" opacity="0.3" stroke="var(--line-strong)" stroke-width="0.8" />
    <text x="332.0" y="196.0" font-size="11" fill="var(--ink)" text-anchor="start" font-weight="400">0</text>
    <rect x="352" y="176" width="28" height="30" fill="var(--sev-info)" opacity="0.3" stroke="var(--line-strong)" stroke-width="0.8" />
    <text x="366.0" y="196.0" font-size="11" fill="var(--ink)" text-anchor="start" font-weight="400">0</text>
    <text x="398.0" y="196.0" font-size="12" fill="var(--ink)" text-anchor="start" font-weight="500">4 bins</text>
    <rect x="40" y="216" width="180" height="30" rx="2" fill="var(--panel)" stroke="var(--line-strong)" stroke-width="1" />
    <text x="52.0" y="236.0" font-size="11.5" fill="var(--ink)" text-anchor="start" font-weight="400">DCT-VIII / DCT-VIII</text>
    <rect x="250" y="216" width="28" height="30" fill="var(--sev-info)" opacity="0.3" stroke="var(--line-strong)" stroke-width="0.8" />
    <text x="264.0" y="236.0" font-size="11" fill="var(--ink)" text-anchor="start" font-weight="400">1</text>
    <rect x="284" y="216" width="28" height="30" fill="var(--sev-info)" opacity="0.3" stroke="var(--line-strong)" stroke-width="0.8" />
    <text x="298.0" y="236.0" font-size="11" fill="var(--ink)" text-anchor="start" font-weight="400">0</text>
    <rect x="318" y="216" width="28" height="30" fill="var(--sev-info)" opacity="0.3" stroke="var(--line-strong)" stroke-width="0.8" />
    <text x="332.0" y="236.0" font-size="11" fill="var(--ink)" text-anchor="start" font-weight="400">0</text>
    <rect x="352" y="216" width="28" height="30" fill="var(--sev-info)" opacity="0.3" stroke="var(--line-strong)" stroke-width="0.8" />
    <text x="366.0" y="236.0" font-size="11" fill="var(--ink)" text-anchor="start" font-weight="400">0</text>
    <text x="398.0" y="236.0" font-size="12" fill="var(--ink)" text-anchor="start" font-weight="500">4 bins</text>
    <rect x="600" y="56" width="300" height="30" rx="2" fill="var(--panel)" stroke="var(--accent)" stroke-width="1.4" />
    <text x="612.0" y="76.0" font-size="11.5" fill="var(--accent)" text-anchor="start" font-weight="400">1 bin: was the prediction right?</text>
    <rect x="600" y="96" width="300" height="30" rx="2" fill="var(--panel)" stroke="var(--line-strong)" stroke-width="1" />
    <text x="612.0" y="116.0" font-size="11.5" fill="var(--ink-dim)" text-anchor="start" font-weight="400">and if not, 2 bins select between the four the prediction ruled out</text>
    <text x="600.0" y="160.0" font-size="11.5" fill="var(--ink-faint)" text-anchor="start" font-weight="400">an override never costs more than the signalling it replaces</text>
    <text x="600.0" y="180.0" font-size="11.5" fill="var(--ink-faint)" text-anchor="start" font-weight="400">the naive form spent a bin on the correction and then the full type again</text>
    <text x="40.0" y="24.0" font-size="12.5" fill="var(--ink-dim)" text-anchor="start" font-weight="400">the inferred mode codes the correction relative to the prediction</text>
    <text x="860.0" y="24.0" font-size="11.5" fill="var(--ink-faint)" text-anchor="end" font-weight="400">the explicit mode is the standard's own mts_idx</text>
    </svg>
  </div>
</figure>

<figure class="chart">
  <figcaption>
    <span>The measurement apparatus: one variable, everything else held constant</span>
    <span class="src">crates/girc-bench/src/main.rs</span>
  </figcaption>
  <div class="plot">
    <svg viewBox="0 0 900 300" role="img" aria-label="A diagram of the benchmark: the same content and quantization parameters through GIRC in both modes, and through three anchor codecs, differing only in how the transform mode reaches the decoder">
    <rect x="40" y="60" width="200" height="46" rx="2" fill="var(--panel)" stroke="var(--line-strong)" stroke-width="1" />
    <text x="54.0" y="88.0" font-size="12" fill="var(--ink)" text-anchor="start" font-weight="400">twelve CIF sequences</text>
    <rect x="40" y="140" width="200" height="46" rx="2" fill="var(--panel)" stroke="var(--line-strong)" stroke-width="1" />
    <text x="54.0" y="168.0" font-size="12" fill="var(--ink)" text-anchor="start" font-weight="400">four quantization parameters</text>
    <rect x="40" y="220" width="200" height="46" rx="2" fill="var(--panel)" stroke="var(--line-strong)" stroke-width="1" />
    <text x="54.0" y="248.0" font-size="12" fill="var(--ink)" text-anchor="start" font-weight="400">ten frames each</text>
    <rect x="320" y="60" width="200" height="76" rx="2" fill="var(--panel)" stroke="var(--ink-dim)" stroke-width="1.4" />
    <text x="334.0" y="90.0" font-size="12.5" fill="var(--ink-dim)" text-anchor="start" font-weight="500">the explicit mode</text>
    <text x="334.0" y="110.0" font-size="11" fill="var(--ink-dim)" text-anchor="start" font-weight="400">the same search, the same quantizer</text>
    <rect x="320" y="160" width="200" height="76" rx="2" fill="var(--panel)" stroke="var(--accent)" stroke-width="1.4" />
    <text x="334.0" y="190.0" font-size="12.5" fill="var(--accent)" text-anchor="start" font-weight="500">the inferred mode</text>
    <text x="334.0" y="210.0" font-size="11" fill="var(--ink-dim)" text-anchor="start" font-weight="400">the same search, plus a margin</text>
    <rect x="580" y="98" width="280" height="84" rx="2" fill="var(--panel)" stroke="var(--sev-critical)" stroke-width="1.4" />
    <text x="594.0" y="124.0" font-size="12.5" fill="var(--sev-critical)" text-anchor="start" font-weight="500">the only difference</text>
    <text x="594.0" y="146.0" font-size="11.5" fill="var(--ink-dim)" text-anchor="start" font-weight="400">how the transform mode reaches the decoder</text>
    <text x="594.0" y="166.0" font-size="11.5" fill="var(--ink-dim)" text-anchor="start" font-weight="400">every other stage is shared by both</text>
    <path d="M 240 83 L 320 98" stroke="var(--line-strong)" stroke-width="1.2" fill="none" />
    <path d="M 520 98 L 580 140" stroke="var(--sev-critical)" stroke-width="1.2" fill="none" />
    <path d="M 240 223 L 320 198" stroke="var(--line-strong)" stroke-width="1.2" fill="none" />
    <path d="M 520 198 L 580 140" stroke="var(--sev-critical)" stroke-width="1.2" fill="none" />
    <text x="40.0" y="24.0" font-size="12" fill="var(--ink-dim)" text-anchor="start" font-weight="400">the anchors are coded the same way, on the same frames, with the same PSNR code</text>
    <text x="40.0" y="286.0" font-size="11.5" fill="var(--ink-faint)" text-anchor="start" font-weight="400">the benchmark decodes what it encodes, and refuses a result if the two pictures differ</text>
    </svg>
  </div>
</figure>

## The chroma planes

No chroma transform type is signalled: H.266 clause 7.3.9 codes a transform type for the luma plane, and the chroma transform type is inferred, and in 4:2:0 it is always the plain DCT-II. The reference encoder agrees — chroma transform selection is gated on `m_useChromaTS`, which is off by default and only enabled for 4:4:4. So the mechanism is a luma mechanism, exactly where the standard signals a transform type at all.

<figure class="chart">
  <figcaption>
    <span>Why there are two rate axes: the mechanism is a luma mechanism</span>
    <span class="src">crates/girc-core/src/chroma.rs, ITU-T H.266 clause 7.3.9</span>
  </figcaption>
  <div class="plot">
    <svg viewBox="0 0 900 280" role="img" aria-label="A 4:2:0 frame drawn to scale: one luma sample per chroma sample in a 2x2 ratio, with the chroma planes carrying no transform-type signalling at all">
    <rect x="40" y="60" width="480" height="320" fill="var(--accent)" opacity="0.14" stroke="var(--line-strong)" stroke-width="1" />
    <text x="48.0" y="84.0" font-size="12" fill="var(--ink)" text-anchor="start" font-weight="400">luma: 352 × 288, four samples per chroma sample</text>
    <text x="48.0" y="104.0" font-size="11.5" fill="var(--ink-dim)" text-anchor="start" font-weight="400">the transform type is signalled here, and the mechanism removes all of it</text>
    <rect x="560" y="100" width="240" height="160" fill="var(--sev-info)" opacity="0.14" stroke="var(--line-strong)" stroke-width="1" />
    <text x="568.0" y="124.0" font-size="12" fill="var(--ink)" text-anchor="start" font-weight="400">Cb: 176 × 144</text>
    <text x="568.0" y="144.0" font-size="11.5" fill="var(--ink-dim)" text-anchor="start" font-weight="400">always the plain DCT-II</text>
    <text x="568.0" y="162.0" font-size="11.5" fill="var(--ink-dim)" text-anchor="start" font-weight="400">no transform type is signalled</text>
    <rect x="690" y="100" width="240" height="160" fill="var(--sev-info)" opacity="0.14" stroke="var(--line-strong)" stroke-width="1" />
    <text x="698.0" y="124.0" font-size="12" fill="var(--ink)" text-anchor="start" font-weight="400">Cr: 176 × 144</text>
    <text x="698.0" y="144.0" font-size="11.5" fill="var(--ink-dim)" text-anchor="start" font-weight="400">always the plain DCT-II</text>
    <text x="698.0" y="162.0" font-size="11.5" fill="var(--ink-dim)" text-anchor="start" font-weight="400">no transform type is signalled</text>
    <text x="40.0" y="24.0" font-size="12.5" fill="var(--ink-dim)" text-anchor="start" font-weight="400">the mechanism changes the luma signalling and nothing else</text>
    <text x="40.0" y="44.0" font-size="11.5" fill="var(--ink-faint)" text-anchor="start" font-weight="400">so the benchmark reports the luma rate, which is the mechanism's own effect, and the whole-frame rate, which is what a deployment saves</text>
    </svg>
  </div>
</figure>

The chroma quantization parameter is mapped, not equal, through H.266's chroma quantization parameter mapping, checked against the reference implementation for every quantization parameter. The chroma grid is four by four chroma samples, one chroma block per luma block, and the chroma motion is derived from the luma motion as H.266 does.

## Against the anchor codecs

GIRC is a research codec with a small mode set and integer-sample motion. It is not a VVC competitor, and this table is context rather than a claim. What the table does show is that the measurement apparatus is real: the same content, the same rate and distortion code, the same frames, and all three planes coded on both sides.

<figure class="chart">
  <figcaption>
    <span>Against the anchors: the same content, the same code, and the scale of the gap</span>
    <span class="src">research/benchmark/report-v9-joint.json · bd_rate_vs_baseline</span>
  </figcaption>
  <div class="plot">
    <svg viewBox="0 0 900 380" role="img" aria-label="Bjontegaard delta rate against the GIRC explicit baseline, per sequence, for x265, SVT-AV1 and VVenC, and for GIRC's own inferred mode">
    <line x1="70.0" y1="294.0" x2="860.0" y2="294.0" stroke="var(--grid)" stroke-width="1.0" />
    <text x="60.0" y="298.0" font-size="11" fill="var(--ink-faint)" text-anchor="end" font-weight="400">-93</text>
    <line x1="70.0" y1="231.5" x2="860.0" y2="231.5" stroke="var(--grid)" stroke-width="1.0" />
    <text x="60.0" y="235.5" font-size="11" fill="var(--ink-faint)" text-anchor="end" font-weight="400">-68</text>
    <line x1="70.0" y1="169.0" x2="860.0" y2="169.0" stroke="var(--grid)" stroke-width="1.0" />
    <text x="60.0" y="173.0" font-size="11" fill="var(--ink-faint)" text-anchor="end" font-weight="400">-43</text>
    <line x1="70.0" y1="106.5" x2="860.0" y2="106.5" stroke="var(--grid)" stroke-width="1.0" />
    <text x="60.0" y="110.5" font-size="11" fill="var(--ink-faint)" text-anchor="end" font-weight="400">-17</text>
    <line x1="70.0" y1="44.0" x2="860.0" y2="44.0" stroke="var(--grid)" stroke-width="1.0" />
    <text x="60.0" y="48.0" font-size="11" fill="var(--ink-faint)" text-anchor="end" font-weight="400">8</text>
    <line x1="70.0" y1="62.8" x2="860.0" y2="62.8" stroke="var(--line-strong)" stroke-width="1.4" />
    <rect x="70.0" y="58.1" width="13.2" height="4.7" fill="var(--accent)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <rect x="85.2" y="57.4" width="13.2" height="5.4" fill="var(--sev-info)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <rect x="100.3" y="62.8" width="13.2" height="183.2" fill="var(--sev-critical)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <rect x="115.5" y="62.8" width="13.2" height="184.2" fill="var(--sev-critical)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <rect x="130.7" y="62.8" width="13.2" height="199.8" fill="var(--sev-critical)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <text x="102.9" y="314.0" font-size="10" fill="var(--ink-faint)" text-anchor="middle" font-weight="400">akiyo</text>
    <rect x="135.8" y="62.8" width="13.2" height="0.0" fill="var(--accent)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <rect x="151.0" y="62.8" width="13.2" height="0.7" fill="var(--sev-info)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <rect x="166.2" y="62.8" width="13.2" height="183.6" fill="var(--sev-critical)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <rect x="181.3" y="62.8" width="13.2" height="189.8" fill="var(--sev-critical)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <rect x="196.5" y="62.8" width="13.2" height="208.2" fill="var(--sev-critical)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <text x="168.7" y="314.0" font-size="10" fill="var(--ink-faint)" text-anchor="middle" font-weight="400">bus</text>
    <rect x="201.7" y="62.8" width="13.2" height="0.8" fill="var(--accent)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <rect x="216.8" y="62.8" width="13.2" height="1.1" fill="var(--sev-info)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <rect x="232.0" y="62.8" width="13.2" height="183.6" fill="var(--sev-critical)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <rect x="247.2" y="62.8" width="13.2" height="189.8" fill="var(--sev-critical)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <rect x="262.3" y="62.8" width="13.2" height="208.2" fill="var(--sev-critical)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <text x="234.6" y="314.0" font-size="10" fill="var(--ink-faint)" text-anchor="middle" font-weight="400">city</text>
    <rect x="267.5" y="62.8" width="13.2" height="1.1" fill="var(--accent)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <rect x="282.7" y="62.8" width="13.2" height="0.0" fill="var(--sev-info)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <rect x="297.8" y="62.8" width="13.2" height="200.2" fill="var(--sev-critical)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <rect x="313.0" y="62.8" width="13.2" height="207.2" fill="var(--sev-critical)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <rect x="328.2" y="62.8" width="13.2" height="217.8" fill="var(--sev-critical)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <text x="300.4" y="314.0" font-size="10" fill="var(--ink-faint)" text-anchor="middle" font-weight="400">coastguard</text>
    <rect x="333.3" y="62.8" width="13.2" height="0.0" fill="var(--accent)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <rect x="348.5" y="62.8" width="13.2" height="0.1" fill="var(--sev-info)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <rect x="363.7" y="62.8" width="13.2" height="200.2" fill="var(--sev-critical)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <rect x="378.8" y="62.8" width="13.2" height="207.2" fill="var(--sev-critical)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <rect x="394.0" y="62.8" width="13.2" height="217.8" fill="var(--sev-critical)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <text x="366.2" y="314.0" font-size="10" fill="var(--ink-faint)" text-anchor="middle" font-weight="400">container</text>
    <rect x="399.2" y="62.8" width="13.2" height="1.0" fill="var(--accent)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <rect x="414.3" y="62.8" width="13.2" height="0.0" fill="var(--sev-info)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <rect x="429.5" y="62.8" width="13.2" height="156.2" fill="var(--sev-critical)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <rect x="444.7" y="62.8" width="13.2" height="175.6" fill="var(--sev-critical)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <rect x="459.8" y="62.8" width="13.2" height="187.6" fill="var(--sev-critical)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <text x="432.1" y="314.0" font-size="10" fill="var(--ink-faint)" text-anchor="middle" font-weight="400">crew</text>
    <rect x="465.0" y="62.8" width="13.2" height="2.0" fill="var(--accent)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <rect x="480.2" y="62.8" width="13.2" height="2.0" fill="var(--sev-info)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <rect x="495.3" y="62.8" width="13.2" height="156.2" fill="var(--sev-critical)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <rect x="510.5" y="62.8" width="13.2" height="175.6" fill="var(--sev-critical)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <rect x="525.7" y="62.8" width="13.2" height="187.6" fill="var(--sev-critical)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <text x="497.9" y="314.0" font-size="10" fill="var(--ink-faint)" text-anchor="middle" font-weight="400">deadline</text>
    <rect x="530.8" y="62.8" width="13.2" height="0.5" fill="var(--accent)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <rect x="546.0" y="62.8" width="13.2" height="1.0" fill="var(--sev-info)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <rect x="561.2" y="62.8" width="13.2" height="185.6" fill="var(--sev-critical)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <rect x="576.3" y="62.8" width="13.2" height="186.3" fill="var(--sev-critical)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <rect x="591.5" y="62.8" width="13.2" height="199.2" fill="var(--sev-critical)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <text x="563.7" y="314.0" font-size="10" fill="var(--ink-faint)" text-anchor="middle" font-weight="400">flower</text>
    <rect x="596.7" y="62.8" width="13.2" height="1.2" fill="var(--accent)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <rect x="611.8" y="62.8" width="13.2" height="0.6" fill="var(--sev-info)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <rect x="627.0" y="62.8" width="13.2" height="185.6" fill="var(--sev-critical)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <rect x="642.2" y="62.8" width="13.2" height="186.3" fill="var(--sev-critical)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <rect x="657.3" y="62.8" width="13.2" height="199.2" fill="var(--sev-critical)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <text x="629.6" y="314.0" font-size="10" fill="var(--ink-faint)" text-anchor="middle" font-weight="400">foreman</text>
    <rect x="662.5" y="62.8" width="13.2" height="0.4" fill="var(--accent)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <rect x="677.7" y="62.8" width="13.2" height="0.5" fill="var(--sev-info)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <rect x="692.8" y="62.8" width="13.2" height="192.3" fill="var(--sev-critical)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <rect x="708.0" y="62.8" width="13.2" height="198.1" fill="var(--sev-critical)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <rect x="723.2" y="62.8" width="13.2" height="212.6" fill="var(--sev-critical)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <text x="695.4" y="314.0" font-size="10" fill="var(--ink-faint)" text-anchor="middle" font-weight="400">highway</text>
    <rect x="728.3" y="62.8" width="13.2" height="0.6" fill="var(--accent)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <rect x="743.5" y="62.8" width="13.2" height="0.5" fill="var(--sev-info)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <rect x="758.7" y="62.8" width="13.2" height="192.3" fill="var(--sev-critical)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <rect x="773.8" y="62.8" width="13.2" height="198.1" fill="var(--sev-critical)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <rect x="789.0" y="62.8" width="13.2" height="212.6" fill="var(--sev-critical)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <text x="761.2" y="314.0" font-size="10" fill="var(--ink-faint)" text-anchor="middle" font-weight="400">mobile</text>
    <rect x="794.2" y="62.8" width="13.2" height="1.6" fill="var(--accent)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <rect x="809.3" y="62.8" width="13.2" height="0.1" fill="var(--sev-info)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <rect x="824.5" y="62.8" width="13.2" height="174.0" fill="var(--sev-critical)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <rect x="839.7" y="62.8" width="13.2" height="170.2" fill="var(--sev-critical)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <rect x="854.8" y="62.8" width="13.2" height="193.5" fill="var(--sev-critical)" stroke="var(--line-strong)" stroke-width="0.5" opacity="0.9" />
    <text x="827.1" y="314.0" font-size="10" fill="var(--ink-faint)" text-anchor="middle" font-weight="400">stefan</text>
    <rect x="70.0" y="20.0" width="12.0" height="9.0" fill="var(--accent)" stroke="var(--line-strong)" stroke-width="0.5" />
    <text x="87.0" y="28.0" font-size="11.5" fill="var(--ink-dim)" text-anchor="start" font-weight="400">girc-inferred</text>
    <rect x="185.0" y="20.0" width="12.0" height="9.0" fill="var(--sev-info)" stroke="var(--line-strong)" stroke-width="0.5" />
    <text x="202.0" y="28.0" font-size="11.5" fill="var(--ink-dim)" text-anchor="start" font-weight="400">girc-constant</text>
    <rect x="300.0" y="20.0" width="12.0" height="9.0" fill="var(--sev-critical)" stroke="var(--line-strong)" stroke-width="0.5" />
    <text x="317.0" y="28.0" font-size="11.5" fill="var(--ink-dim)" text-anchor="start" font-weight="400">x265</text>
    <rect x="352.0" y="20.0" width="12.0" height="9.0" fill="var(--sev-critical)" stroke="var(--line-strong)" stroke-width="0.5" />
    <text x="369.0" y="28.0" font-size="11.5" fill="var(--ink-dim)" text-anchor="start" font-weight="400">svt-av1</text>
    <rect x="425.0" y="20.0" width="12.0" height="9.0" fill="var(--sev-critical)" stroke="var(--line-strong)" stroke-width="0.5" />
    <text x="442.0" y="28.0" font-size="11.5" fill="var(--ink-dim)" text-anchor="start" font-weight="400">vvenc</text>
    <text x="70.0" y="24.0" font-size="12" fill="var(--ink-dim)" text-anchor="start" font-weight="400">negative is a smaller bitrate than the GIRC explicit mode</text>
    </svg>
  </div>
</figure>

<figure class="chart">
  <figcaption>
    <span>Rate-distortion, foreman_cif: the two GIRC modes and the three anchors</span>
    <span class="src">research/benchmark/report-v9-joint.json · results</span>
  </figcaption>
  <div class="plot">
    <svg viewBox="0 0 900 400" role="img" aria-label="The rate-distortion curves for foreman_cif at four quantization parameters, for GIRC in both modes and for x265, SVT-AV1 and VVenC">
    <line x1="80.0" y1="295.4" x2="820.0" y2="295.4" stroke="var(--grid)" stroke-width="1.0" />
    <text x="70.0" y="299.4" font-size="11" fill="var(--ink-faint)" text-anchor="end" font-weight="400">33.0</text>
    <line x1="80.0" y1="237.2" x2="820.0" y2="237.2" stroke="var(--grid)" stroke-width="1.0" />
    <text x="70.0" y="241.2" font-size="11" fill="var(--ink-faint)" text-anchor="end" font-weight="400">35.4</text>
    <line x1="80.0" y1="179.0" x2="820.0" y2="179.0" stroke="var(--grid)" stroke-width="1.0" />
    <text x="70.0" y="183.0" font-size="11" fill="var(--ink-faint)" text-anchor="end" font-weight="400">37.7</text>
    <line x1="80.0" y1="120.8" x2="820.0" y2="120.8" stroke="var(--grid)" stroke-width="1.0" />
    <text x="70.0" y="124.8" font-size="11" fill="var(--ink-faint)" text-anchor="end" font-weight="400">40.1</text>
    <line x1="80.0" y1="62.6" x2="820.0" y2="62.6" stroke="var(--grid)" stroke-width="1.0" />
    <text x="70.0" y="66.6" font-size="11" fill="var(--ink-faint)" text-anchor="end" font-weight="400">42.5</text>
    <polyline points="820.0,89.1 559.6,167.0 367.1,235.4 255.4,295.3" fill="none" stroke="var(--ink-dim)" stroke-width="2.0" />
    <circle cx="820.0" cy="89.1" r="3.5" fill="var(--ink-dim)" />
    <circle cx="559.6" cy="167.0" r="3.5" fill="var(--ink-dim)" />
    <circle cx="367.1" cy="235.4" r="3.5" fill="var(--ink-dim)" />
    <circle cx="255.4" cy="295.3" r="3.5" fill="var(--ink-dim)" />
    <polyline points="817.0,90.5 555.2,167.4 364.4,235.4 255.7,295.4" fill="none" stroke="var(--accent)" stroke-width="2.0" />
    <circle cx="817.0" cy="90.5" r="3.5" fill="var(--accent)" />
    <circle cx="555.2" cy="167.4" r="3.5" fill="var(--accent)" />
    <circle cx="364.4" cy="235.4" r="3.5" fill="var(--accent)" />
    <circle cx="255.7" cy="295.4" r="3.5" fill="var(--accent)" />
    <polyline points="302.4,93.6 181.5,160.4 121.9,230.6 91.0,295.2" fill="none" stroke="var(--sev-info)" stroke-width="2.0" />
    <circle cx="302.4" cy="93.6" r="3.5" fill="var(--sev-info)" />
    <circle cx="181.5" cy="160.4" r="3.5" fill="var(--sev-info)" />
    <circle cx="121.9" cy="230.6" r="3.5" fill="var(--sev-info)" />
    <circle cx="91.0" cy="295.2" r="3.5" fill="var(--sev-info)" />
    <polyline points="239.7,108.8 195.0,134.9 158.6,162.1 134.1,186.7" fill="none" stroke="var(--sev-warning)" stroke-width="2.0" />
    <circle cx="239.7" cy="108.8" r="3.5" fill="var(--sev-warning)" />
    <circle cx="195.0" cy="134.9" r="3.5" fill="var(--sev-warning)" />
    <circle cx="158.6" cy="162.1" r="3.5" fill="var(--sev-warning)" />
    <circle cx="134.1" cy="186.7" r="3.5" fill="var(--sev-warning)" />
    <polyline points="268.8,62.6 159.5,128.0 105.3,194.0 80.0,250.8" fill="none" stroke="var(--sev-ok)" stroke-width="2.0" />
    <circle cx="268.8" cy="62.6" r="3.5" fill="var(--sev-ok)" />
    <circle cx="159.5" cy="128.0" r="3.5" fill="var(--sev-ok)" />
    <circle cx="105.3" cy="194.0" r="3.5" fill="var(--sev-ok)" />
    <circle cx="80.0" cy="250.8" r="3.5" fill="var(--sev-ok)" />
    <line x1="80.0" y1="26.0" x2="98.0" y2="26.0" stroke="var(--ink-dim)" stroke-width="2.2" />
    <text x="104.0" y="30.0" font-size="11.5" fill="var(--ink-dim)" text-anchor="start" font-weight="400">girc-explicit</text>
    <line x1="201.0" y1="26.0" x2="219.0" y2="26.0" stroke="var(--accent)" stroke-width="2.2" />
    <text x="225.0" y="30.0" font-size="11.5" fill="var(--ink-dim)" text-anchor="start" font-weight="400">girc-inferred</text>
    <line x1="322.0" y1="26.0" x2="340.0" y2="26.0" stroke="var(--sev-info)" stroke-width="2.2" />
    <text x="346.0" y="30.0" font-size="11.5" fill="var(--ink-dim)" text-anchor="start" font-weight="400">x265</text>
    <line x1="380.0" y1="26.0" x2="398.0" y2="26.0" stroke="var(--sev-warning)" stroke-width="2.2" />
    <text x="404.0" y="30.0" font-size="11.5" fill="var(--ink-dim)" text-anchor="start" font-weight="400">svt-av1</text>
    <line x1="459.0" y1="26.0" x2="477.0" y2="26.0" stroke="var(--sev-ok)" stroke-width="2.2" />
    <text x="483.0" y="30.0" font-size="11.5" fill="var(--ink-dim)" text-anchor="start" font-weight="400">vvenc</text>
    <line x1="367.1" y1="235.4" x2="364.4" y2="235.4" stroke="var(--sev-critical)" stroke-width="1.6" />
    <text x="365.8" y="261.4" font-size="11" fill="var(--sev-critical)" text-anchor="middle" font-weight="400">the mechanism, at QP 32: 3968 bits apart</text>
    <text x="18.0" y="28.0" font-size="11.5" fill="var(--ink-dim)" text-anchor="start" font-weight="400">PSNR, dB</text>
    <text x="450.0" y="354.0" font-size="11.5" fill="var(--ink-dim)" text-anchor="middle" font-weight="400">bits for ten frames</text>
    <text x="80.0" y="24.0" font-size="12" fill="var(--ink-faint)" text-anchor="start" font-weight="400">the anchors are better codecs; the gap between the two GIRC lines is the mechanism</text>
    </svg>
  </div>
</figure>

The BD-rate of each anchor against the GIRC-explicit baseline is −73.34% for x265, −73.72% for SVT-AV1 and −79.98% for VVenC: the anchors are better codecs, as expected, and the figure is the scale of the gap. The gap between the two GIRC lines is the mechanism.

## The pipeline

<figure class="chart">
  <figcaption>
    <span>The pipeline: where each artifact comes from, and where it is used</span>
    <span class="src">the repository layout</span>
  </figcaption>
  <div class="plot">
    <svg viewBox="0 0 900 320" role="img" aria-label="A diagram of the repository: the crate that implements the codec, the pipeline that trains the predictor on the encoder's own decisions, and the harness that measures both">
    <rect x="40" y="60" width="270" height="196" rx="2" fill="var(--panel)" stroke="var(--accent)" stroke-width="1.4" />
    <rect x="40" y="60" width="270" height="34" fill="var(--accent)" opacity="0.16" />
    <text x="54.0" y="82.0" font-size="12.5" fill="var(--accent)" text-anchor="start" font-weight="500">crates/girc-core</text>
    <text x="54.0" y="118.0" font-size="11.5" fill="var(--ink-dim)" text-anchor="start" font-weight="400">VVC-exact integer transforms</text>
    <text x="54.0" y="152.0" font-size="11.5" fill="var(--ink-dim)" text-anchor="start" font-weight="400">intra and inter prediction</text>
    <text x="54.0" y="186.0" font-size="11.5" fill="var(--ink-dim)" text-anchor="start" font-weight="400">the geometry predictor</text>
    <text x="54.0" y="220.0" font-size="11.5" fill="var(--ink-dim)" text-anchor="start" font-weight="400">the entropy coder</text>
    <rect x="340" y="60" width="270" height="196" rx="2" fill="var(--panel)" stroke="var(--sev-info)" stroke-width="1.4" />
    <rect x="340" y="60" width="270" height="34" fill="var(--sev-info)" opacity="0.16" />
    <text x="354.0" y="82.0" font-size="12.5" fill="var(--sev-info)" text-anchor="start" font-weight="500">python/girc-train</text>
    <text x="354.0" y="118.0" font-size="11.5" fill="var(--ink-dim)" text-anchor="start" font-weight="400">collect: the encoder's decisions</text>
    <text x="354.0" y="152.0" font-size="11.5" fill="var(--ink-dim)" text-anchor="start" font-weight="400">fit: one model over five modes</text>
    <text x="354.0" y="186.0" font-size="11.5" fill="var(--ink-dim)" text-anchor="start" font-weight="400">the predictability ceiling</text>
    <text x="354.0" y="220.0" font-size="11.5" fill="var(--ink-dim)" text-anchor="start" font-weight="400">a frozen, versioned artifact</text>
    <rect x="640" y="60" width="270" height="196" rx="2" fill="var(--panel)" stroke="var(--sev-ok)" stroke-width="1.4" />
    <rect x="640" y="60" width="270" height="34" fill="var(--sev-ok)" opacity="0.16" />
    <text x="654.0" y="82.0" font-size="12.5" fill="var(--sev-ok)" text-anchor="start" font-weight="500">crates/girc-bench</text>
    <text x="654.0" y="118.0" font-size="11.5" fill="var(--ink-dim)" text-anchor="start" font-weight="400">both modes, same content</text>
    <text x="654.0" y="152.0" font-size="11.5" fill="var(--ink-dim)" text-anchor="start" font-weight="400">the margin sweep</text>
    <text x="654.0" y="186.0" font-size="11.5" fill="var(--ink-dim)" text-anchor="start" font-weight="400">the anchor codecs</text>
    <text x="654.0" y="220.0" font-size="11.5" fill="var(--ink-dim)" text-anchor="start" font-weight="400">the Bjontegaard implementation</text>
    <rect x="70" y="290" width="810" height="46" rx="2" fill="var(--panel)" stroke="var(--accent)" stroke-width="1.4" />
    <text x="84.0" y="318.0" font-size="12" fill="var(--accent)" text-anchor="start" font-weight="400">models/girc-predictor.json — produced by the middle, read by both sides of the codec</text>
    <path d="M 310 210 L 340 210" stroke="var(--line-strong)" stroke-width="1.4" fill="none" marker-end="url(#arrow4)" />
    <path d="M 610 210 L 640 210" stroke="var(--line-strong)" stroke-width="1.4" fill="none" marker-end="url(#arrow4)" />
    <defs><marker id="arrow4" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M 0 0 L 10 5 L 0 10 z" fill="var(--line-strong)" /></marker></defs>
    <text x="40.0" y="24.0" font-size="12" fill="var(--ink-dim)" text-anchor="start" font-weight="400">one product: the codec, the training pipeline, the benchmark, and the portal that evaluates it</text>
    </svg>
  </div>
</figure>

## What is held constant

The two modes share the intra prediction, the quantization, the transform arithmetic, the residual coding and the entropy coder. They differ in one place: how the transform type reaches the decoder. A difference in the measured rate is therefore attributable to the signalling mechanism alone.

The transform arithmetic is the real thing. The integer transforms are bit-exact against Fraunhofer HHI's VVenC 1.14.0, checked by a test that drives the reference implementation's own kernels and compares the results bit for bit across all twelve transform and size combinations.

## The portal and the hosted analysis

The portal is the confidential evaluation environment: organisation accounts, a versioned agreement, gated artifact delivery and an append-only audit log. The hosted analysis is the other half: post a sequence and get the same figures the benchmark writes, without building a toolchain first.

<figure class="chart">
  <figcaption>
    <span>The evaluation lifecycle, as the portal implements it</span>
    <span class="src">apps/portal/girc_eval/app.py</span>
  </figcaption>
  <div class="plot">
    <svg viewBox="0 0 900 300" role="img" aria-label="The five states of an evaluation: registered, agreed, requested, approved, evaluated, with the gated artifacts released only between approved and evaluated">
    <rect x="40.0" y="100" width="146.0" height="54" rx="2" fill="var(--panel)" stroke="var(--line-strong)" stroke-width="1" />
    <rect x="40.0" y="100" width="4" height="54" fill="var(--accent)" />
    <text x="54.0" y="124.0" font-size="12.5" fill="var(--ink)" text-anchor="start" font-weight="500">registered</text>
    <text x="54.0" y="142.0" font-size="10.5" fill="var(--ink-faint)" text-anchor="start" font-weight="400">an organisation account exists</text>
    <path d="M 186.0 127 H 202.0" stroke="var(--line-strong)" stroke-width="1.4" fill="none" marker-end="url(#arrow3)" />
    <rect x="204.0" y="100" width="146.0" height="54" rx="2" fill="var(--panel)" stroke="var(--line-strong)" stroke-width="1" />
    <rect x="204.0" y="100" width="4" height="54" fill="var(--accent)" />
    <text x="218.0" y="124.0" font-size="12.5" fill="var(--ink)" text-anchor="start" font-weight="500">agreed</text>
    <text x="218.0" y="142.0" font-size="10.5" fill="var(--ink-faint)" text-anchor="start" font-weight="400">the agreement's version and digest are recorded</text>
    <path d="M 350.0 127 H 366.0" stroke="var(--line-strong)" stroke-width="1.4" fill="none" marker-end="url(#arrow3)" />
    <rect x="368.0" y="100" width="146.0" height="54" rx="2" fill="var(--panel)" stroke="var(--line-strong)" stroke-width="1" />
    <rect x="368.0" y="100" width="4" height="54" fill="var(--sev-info)" />
    <text x="382.0" y="124.0" font-size="12.5" fill="var(--ink)" text-anchor="start" font-weight="500">requested</text>
    <text x="382.0" y="142.0" font-size="10.5" fill="var(--ink-faint)" text-anchor="start" font-weight="400">the codec and the content are named</text>
    <path d="M 514.0 127 H 530.0" stroke="var(--line-strong)" stroke-width="1.4" fill="none" marker-end="url(#arrow3)" />
    <rect x="532.0" y="100" width="146.0" height="54" rx="2" fill="var(--panel)" stroke="var(--line-strong)" stroke-width="1" />
    <rect x="532.0" y="100" width="4" height="54" fill="var(--sev-ok)" />
    <text x="546.0" y="124.0" font-size="12.5" fill="var(--ink)" text-anchor="start" font-weight="500">approved</text>
    <text x="546.0" y="142.0" font-size="10.5" fill="var(--ink-faint)" text-anchor="start" font-weight="400">a reviewer approves; downloads are signed</text>
    <path d="M 678.0 127 H 694.0" stroke="var(--line-strong)" stroke-width="1.4" fill="none" marker-end="url(#arrow3)" />
    <rect x="696.0" y="100" width="146.0" height="54" rx="2" fill="var(--panel)" stroke="var(--line-strong)" stroke-width="1" />
    <rect x="696.0" y="100" width="4" height="54" fill="var(--ink-faint)" />
    <text x="710.0" y="124.0" font-size="12.5" fill="var(--ink)" text-anchor="start" font-weight="500">evaluated</text>
    <text x="710.0" y="142.0" font-size="10.5" fill="var(--ink-faint)" text-anchor="start" font-weight="400">results are submitted and accepted</text>
    <line x1="522.0" y1="44" x2="522.0" y2="210" stroke="var(--sev-critical)" stroke-width="1.4" stroke-dasharray="5 4" />
    <text x="516.0" y="36.0" font-size="11.5" fill="var(--sev-critical)" text-anchor="end" font-weight="400">the gate: nothing crosses it unsigned</text>
    <defs><marker id="arrow3" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M 0 0 L 10 5 L 0 10 z" fill="var(--line-strong)" /></marker></defs>
    <text x="40.0" y="24.0" font-size="12" fill="var(--ink-dim)" text-anchor="start" font-weight="400">every transition writes an audit entry: the actor, the time and the address it came from</text>
    <text x="40.0" y="44.0" font-size="11.5" fill="var(--ink-faint)" text-anchor="start" font-weight="400">a download token is signed, time-limited, bound to the artifact's digest and to the requesting user</text>
    <text x="40.0" y="230.0" font-size="12" fill="var(--ink-dim)" text-anchor="start" font-weight="400">the audit log is append-only: the application never updates or deletes a row in it</text>
    </svg>
  </div>
</figure>

<figure class="chart">
  <figcaption>
    <span>The hosted analysis: the same code, over HTTP, on one free instance</span>
    <span class="src">apps/portal, render.yaml</span>
  </figcaption>
  <div class="plot">
    <svg viewBox="0 0 900 300" role="img" aria-label="A diagram of the hosted service: the browser posts a trace, the portal encodes and decodes it, and the report comes back, with no trace retained">
    <rect x="40" y="70" width="220" height="100" rx="2" fill="var(--panel)" stroke="var(--ink-dim)" stroke-width="1.4" />
    <rect x="40" y="70" width="220" height="30" fill="var(--ink-dim)" opacity="0.16" />
    <text x="54.0" y="90.0" font-size="12" fill="var(--ink-dim)" text-anchor="start" font-weight="500">you</text>
    <text x="54.0" y="126.0" font-size="11.5" fill="var(--ink-dim)" text-anchor="start" font-weight="400">paste a sequence</text>
    <text x="54.0" y="148.0" font-size="11.5" fill="var(--ink-dim)" text-anchor="start" font-weight="400">or upload a .girc stream</text>
    <rect x="330" y="70" width="220" height="100" rx="2" fill="var(--panel)" stroke="var(--accent)" stroke-width="1.4" />
    <rect x="330" y="70" width="220" height="30" fill="var(--accent)" opacity="0.16" />
    <text x="344.0" y="90.0" font-size="12" fill="var(--accent)" text-anchor="start" font-weight="500">the portal, on Render</text>
    <text x="344.0" y="126.0" font-size="11.5" fill="var(--ink-dim)" text-anchor="start" font-weight="400">encode, decode, measure</text>
    <text x="344.0" y="148.0" font-size="11.5" fill="var(--ink-dim)" text-anchor="start" font-weight="400">nothing is stored</text>
    <rect x="620" y="70" width="240" height="100" rx="2" fill="var(--panel)" stroke="var(--sev-ok)" stroke-width="1.4" />
    <rect x="620" y="70" width="240" height="30" fill="var(--sev-ok)" opacity="0.16" />
    <text x="634.0" y="90.0" font-size="12" fill="var(--sev-ok)" text-anchor="start" font-weight="500">the report</text>
    <text x="634.0" y="126.0" font-size="11.5" fill="var(--ink-dim)" text-anchor="start" font-weight="400">the same figures</text>
    <text x="634.0" y="148.0" font-size="11.5" fill="var(--ink-dim)" text-anchor="start" font-weight="400">as the local run</text>
    <path d="M 260 120 H 330" stroke="var(--line-strong)" stroke-width="1.4" fill="none" marker-end="url(#arrow5)" />
    <path d="M 550 120 H 620" stroke="var(--line-strong)" stroke-width="1.4" fill="none" marker-end="url(#arrow5)" />
    <defs><marker id="arrow5" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M 0 0 L 10 5 L 0 10 z" fill="var(--line-strong)" /></marker></defs>
    <text x="40.0" y="24.0" font-size="12" fill="var(--ink-dim)" text-anchor="start" font-weight="400">a free instance sleeps after fifteen minutes idle, so an uptime monitor pings it every five</text>
    <text x="40.0" y="44.0" font-size="11.5" fill="var(--ink-faint)" text-anchor="start" font-weight="400">one service, on purpose: a workspace gets 750 free hours a month, and a month is 730 to 744 hours</text>
    <text x="40.0" y="226.0" font-size="12.5" fill="var(--sev-critical)" text-anchor="start" font-weight="500">what it is not</text>
    <text x="40.0" y="250.0" font-size="11.5" fill="var(--ink-dim)" text-anchor="start" font-weight="400">not a different product: the portal runs the same crate you can read and deploy yourself</text>
    <text x="40.0" y="270.0" font-size="11.5" fill="var(--ink-dim)" text-anchor="start" font-weight="400">not a hosted-only feature: every figure it returns is reproducible from the repository</text>
    </svg>
  </div>
</figure>

The portal runs on one free Render instance, because a workspace gets 750 free instance hours a month and a month is 730 to 744 hours, so exactly one always-on free service fits. An UptimeRobot monitor pings `/health` every five minutes to keep the instance awake, since a free service sleeps after fifteen minutes idle.

