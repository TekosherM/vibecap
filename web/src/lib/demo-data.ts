export const DEMO_CONSOLE = [
  {
    level: "error" as const,
    message:
      "Uncaught TypeError: Cannot read properties of undefined (reading 'tax')",
    meta: "checkout.js:214",
  },
  {
    level: "warn" as const,
    message: "React does not recognize the `onPay` prop on a DOM element.",
    meta: "CartFooter.tsx:41",
  },
  {
    level: "info" as const,
    message: "POST /api/checkout 402 Payment Required 184ms",
    meta: "network",
  },
  {
    level: "error" as const,
    message: "PaymentIntent failed: card_declined (generic_decline)",
    meta: "stripe.js",
  },
];

export const DEMO_HTTP = [
  {
    method: "GET",
    path: "/api/cart",
    status: 200,
    ms: 42,
    body: '{"items":3,"subtotal":4500}',
  },
  {
    method: "POST",
    path: "/api/checkout",
    status: 402,
    ms: 184,
    body: '{"error":"card_declined","code":"generic_decline"}',
  },
  {
    method: "GET",
    path: "/api/tax?zip=94107",
    status: 500,
    ms: 12,
    body: '{"error":"tax is undefined"}',
  },
];

export const DEMO_TERMINAL = [
  "$ docker compose logs checkout --tail 20",
  "checkout-1  | 05:11:02  INFO  listening on :8088",
  "checkout-1  | 05:11:18  INFO  GET /api/cart 200 42ms",
  "checkout-1  | 05:11:19  ERROR TypeError: Cannot read properties of undefined (reading 'tax')",
  "checkout-1  |     at computeTotal (pricing.ts:88:19)",
  "checkout-1  |     at POST /api/checkout (routes/checkout.ts:41:12)",
  "checkout-1  | 05:11:19  WARN  stripe.paymentIntents.create → 402 card_declined",
  "checkout-1  | 05:11:19  INFO  POST /api/checkout 402 184ms",
  "",
  "$ psql $DATABASE_URL -c 'select sku, price_cents, stock from catalog_items;'",
  "  sku  | price_cents | stock",
  "-------+-------------+-------",
  " LM-12 |        1200 |    18",
  " LM-18 |        1800 |     7",
  " LM-15 |        1500 |     0",
  "",
  "$ echo $CHECKOUT_TOTAL_BUG",
  "UI renders $41.00 — items sum to $45.00 (tax helper threw, fallback omitted tote)",
].join("\n");

export const DEMO_FRONTEND_DOM = {
  url: "https://checkout.lumen.test/pay",
  title: "Lumen Cart — checkout",
  viewport: { w: 1280, h: 720 },
  issues: [
    {
      id: "total-mismatch",
      detail: "Order total shows $41.00; line items 12+18+15 = $45.00",
    },
    {
      id: "overflow",
      detail: "Primary CTA label clipped: “Pay now — encrypted”",
    },
    {
      id: "contrast",
      detail: "Muted helper text #c4b8a8 on #f4efe6 fails WCAG AA",
    },
    {
      id: "stock",
      detail: "Graphite notebook is in cart with stock 0",
    },
  ],
  headings: ["Order summary", "Pay with card", "Apply coupon"],
};

export function formatMoney(cents: number) {
  return `$${(cents / 100).toFixed(2)}`;
}
