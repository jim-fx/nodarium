type ExtractValues<T> = {
  [K in keyof T]: T[K] extends { value: infer V } ? V
    : T[K] extends object ? ExtractValues<T[K]>
    : never;
};
