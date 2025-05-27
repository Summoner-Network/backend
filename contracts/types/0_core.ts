export type u64 = bigint;

export interface Elm {
    readonly index: number;
    readonly id: u64 | undefined;
}

type Pointer<T extends Elm> = [typeIndex: number, address: u64];

const enum EdgeType {
    ElmAccountFactors = "has_factor",
    ElmFactorSecures = "is_securing",
    FactorDelegates = "is_delegating",
    FactorSubordinates = "is_suborning",
}

export const enum ElmType {
    Account = 1000,
    AccountFactorNormal = 1001,
    AccountFactorCrypto = 1002,
    Balance = 2000,
    Payment = 2001,
    Exchange = 2002,
    Asset = 2003,

    Index = 3000,
}