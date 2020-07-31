use crate::{
  poly::*,
  polyvec::*,
  rng::*,
  ntt::*,
  symmetric::*,
  params::*;
};

/*************************************************
* Name:        pack_pk
*
* Description: Serialize the public key as concatenation of the
*              serialized vector of polynomials pk
*              and the public seed used to generate the matrix A.
*
* Arguments:   unsigned char *r:          pointer to the output serialized public key
*              const poly *pk:            pointer to the input public-key polynomial
*              const unsigned char *seed: pointer to the input public seed
**************************************************/
pub fn pack_pk(r: &mut[u8], pk: &Polyvec, seed: &[u8])
{
  polyvec_tobytes(r, pk);
  for i in 0..KYBER_SYMBYTES {
    r[i+KYBER_POLYVECBYTES] = seed[i];
  }
}

/*************************************************
* Name:        unpack_pk
*
* Description: De-serialize public key from a byte array;
*              approximate inverse of pack_pk
*
* Arguments:   - polyvec *pk:                   pointer to output public-key vector of polynomials
*              - unsigned char *seed:           pointer to output seed to generate matrix A
*              - const unsigned char *packedpk: pointer to input serialized public key
**************************************************/
pub fn unpack_pk(pk: &mut Polyvec, seed: &mut[u8], packedpk: &[u8])
{
  
  polyvec_frombytes(pk, packedpk);
  for i in 0..KYBER_SYMBYTES {
    seed[i] = packedpk[i+KYBER_POLYVECBYTES];
  }
}

/*************************************************
* Name:        pack_sk
*
* Description: Serialize the secret key
*
* Arguments:   - unsigned char *r:  pointer to output serialized secret key
*              - const polyvec *sk: pointer to input vector of polynomials (secret key)
**************************************************/
pub fn pack_sk(r: &mut[u8], sk: &Polyvec)
{
  polyvec_tobytes(&mut r, sk);
}

/*************************************************
* Name:        unpack_sk
*
* Description: De-serialize the secret key;
*              inverse of pack_sk
*
* Arguments:   - polyvec *sk:                   pointer to output vector of polynomials (secret key)
*              - const unsigned char *packedsk: pointer to input serialized secret key
**************************************************/
pub fn unpack_sk(sk: &mut Polyvec, packedsk: &[u8])
{
  polyvec_frombytes(&mut sk, packedsk);
}


/*************************************************
* Name:        pack_ciphertext
*
* Description: Serialize the ciphertext as concatenation of the
*              compressed and serialized vector of polynomials b
*              and the compressed and serialized polynomial v
*
* Arguments:   unsigned char *r:          pointer to the output serialized ciphertext
*              const poly *pk:            pointer to the input vector of polynomials b
*              const unsigned char *seed: pointer to the input polynomial v
**************************************************/
pub fn pack_ciphertext(r: &mut[u8], b: &Polyvec, v: &Poly)
{
  polyvec_compress(r, b);
  poly_compress(r[KYBER_POLYVECCOMPRESSEDBYTES..], v);
}


/*************************************************
* Name:        unpack_ciphertext
*
* Description: De-serialize and decompress ciphertext from a byte array;
*              approximate inverse of pack_ciphertext
*
* Arguments:   - polyvec *b:             pointer to the output vector of polynomials b
*              - poly *v:                pointer to the output polynomial v
*              - const unsigned char *c: pointer to the input serialized ciphertext
**************************************************/
pub fn unpack_ciphertext(b: &mut Polyvec, v: &mut Poly, c: &[u8])
{
  polyvec_decompress(b, c);
  poly_decompress(v, &c[KYBER_POLYVECCOMPRESSEDBYTES..]);
}

/*************************************************
* Name:        rej_uniform
*
* Description: Run rejection sampling on uniform random bytes to generate
*              uniform random integers mod q
*
* Arguments:   - int16_t *r:               pointer to output buffer
*              - unsigned int len:         requested number of 16-bit integers (uniform mod q)
*              - const unsigned char *buf: pointer to input buffer (assumed to be uniform random bytes)
*              - unsigned int buflen:      length of input buffer in bytes
*
* Returns number of sampled 16-bit integers (at most len)
**************************************************/
pub fn rej_uniform(r: &mut[i16], len: usize, buf: &[u8], buflen: usize) -> usize
{
  let (mut ctr, mut pos) = (0usize, 0usize);
  let mut val = 0u16;

  while ctr < len && pos + 2 <= buflen {
    
    val = (buf[pos] | (buf[pos+1] << 8)) as u16;
    pos += 2;

    if val < 19*KYBER_Q as u16
    {
      val -= (val >> 12) * KYBER_Q as u16; // Barrett reduction
      r[ctr] = val as i16;
    }
  }
  ctr
}

pub fn gen_a(a: &mut Polyvec, b: &[u8]) 
{
  gen_matrix(a, b, false);
}


pub fn gen_at(a: &mut Polyvec, b: &[u8]) 
{
  gen_matrix(a, b, true);
}


/*************************************************
* Name:        gen_matrix
*
* Description: Deterministically generate matrix A (or the transpose of A)
*              from a seed. Entries of the matrix are polynomials that look
*              uniformly random. Performs rejection sampling on output of
*              a XOF
*
* Arguments:   - polyvec *a:                pointer to ouptput matrix A
*              - const unsigned char *seed: pointer to input seed
*              - int transposed:            boolean deciding whether A or A^T is generated
**************************************************/
pub fn gen_matrix(a: &mut Polyvec, seed: &[u8], transposed: bool)
{ 
  let mut ctr = 0usize;
  let maxnblocks = (530+XOF_BLOCKBYTES)/XOF_BLOCKBYTES; /* 530 is expected number of required bytes */
  let mut buf = [0u8; XOF_BLOCKBYTES*maxnblocks+1]

  let mut state = xof_state::new();

  for i in 0..KYBER_K {
    for j in 0..KYBER_K {
      if transposed {
        xof_absorb(&state, seed, i, j);
      }
      else {
        xof_absorb(&state, seed, j, i);
      }
      xof_squeezeblocks(buf, maxnblocks, &state);
      ctr = rej_uniform(a[i].vec[j].coeffs, KYBER_N, buf, maxnblocks*XOF_BLOCKBYTES);

      while ctr < KYBER_N
      {
        xof_squeezeblocks(buf, 1, &state);
        ctr += rej_uniform(a[i].vec[j].coeffs + ctr, KYBER_N - ctr, buf, XOF_BLOCKBYTES);
      }
    }
  }
}


/*************************************************
* Name:        indcpa_keypair
*
* Description: Generates public and private key for the CPA-secure
*              public-key encryption scheme underlying Kyber
*
* Arguments:   - unsigned char *pk: pointer to output public key (of length KYBER_INDCPA_PUBLICKEYBYTES bytes)
*              - unsigned char *sk: pointer to output private key (of length KYBER_INDCPA_SECRETKEYBYTES bytes)
**************************************************/
pub fn indcpa_keypair(pk : &mut[u8], sk: &mut[u8]) 
{
  let mut a = [Polyvec; KYBER_K];
  let (mut e, mut pkpv, mut skpv) = (Polyvec::new(), Polyvec::new(), Polyvec::new());
  let mut buf = [0u8; 2*KYBER_SYMBYTES];
  let (publicseed, noiseseed) = buf.split_at_mut(KYBER_SYMBYTES);
  let mut nonce = 0u8;
  
  randombytes(&mut buf, KYBER_SYMBYTES);
  hash_g(&mut buf, buf, KYBER_SYMBYTES);

  gen_a(a, publicseed);

  for i in 0..KYBER_K {
    poly_getnoise(skpv.vec+i, noiseseed, nonce);
    nonce += 1;
  }
  for i in 0..KYBER_K {
    poly_getnoise(e.vec+i, noiseseed, nonce);
    nonce += 1;
  }
  
  polyvec_ntt(&skpv);
  polyvec_ntt(&e);

  // matrix-vector multiplication
  for i in 0..KYBER_K {
    polyvec_pointwise_acc(&pkpv.vec[i], &a[i], &skpv);
    poly_frommont(&pkpv.vec[i]);
  }
  
  polyvec_add(&pkpv, &pkpv, &e);
  polyvec_reduce(&pkpv);

  pack_sk(sk, &skpv);
  pack_pk(pk, &pkpv, publicseed);
}


/*************************************************
* Name:        indcpa_enc
*
* Description: Encryption function of the CPA-secure
*              public-key encryption scheme underlying Kyber.
*
* Arguments:   - unsigned char *c:          pointer to output ciphertext (of length KYBER_INDCPA_BYTES bytes)
*              - const unsigned char *m:    pointer to input message (of length KYBER_INDCPA_MSGBYTES bytes)
*              - const unsigned char *pk:   pointer to input public key (of length KYBER_INDCPA_PUBLICKEYBYTES bytes)
*              - const unsigned char *coin: pointer to input random coins used as seed (of length KYBER_SYMBYTES bytes)
*                                           to deterministically generate all randomness
**************************************************/
pub fn indcpa_enc(c: &mut[u8], m: &[u8], pk: &[u8], coins: &[u8])
{
  let mut at = [Polyvec; KYBER_K];
  let (mut sp, mut pkpv, mut ep, mut bp) = (Polyvec::new(),Polyvec::new(), Polyvec::new(), Polyvec::new());
  let (mut v, mut k, mut epp) = (Poly::new(), Poly::new(), Poly::new());
  let mut seed = [0u8; KYBER_SYMBYTES];
  let mut nonce = 0u8;
  
  unpack_pk(&pkpv, &mut seed, pk);
  poly_frommsg(&mut k, m);
  gen_at(at, &seed);

  for i in 0..KYBER_K {
    poly_getnoise(sp.vec+i, coins, nonce);
    nonce += 1;
  }
  for i in 0..KYBER_K {
    poly_getnoise(sp.vec+i, coins, nonce);
    nonce += 1;
  }

  polyvec_ntt(&sp);

  // matrix-vector multiplication
  for i in 0..KYBER_K {    
    polyvec_pointwise_acc(&bp.vec[i], &at[i], &sp);
  }

  polyvec_pointwise_acc(&v, &pkpv, &sp);

  polyvec_invntt(&bp);
  poly_invntt(&mut v);

  polyvec_add(&bp, &bp, &ep);
  poly_add(&mut v, &v, epp);
  poly_add(&mut v, &v, k);
  polyvec_reduce(&bp);
  poly_reduce(&mut v);

  pack_ciphertext(c, &bp, &v);
}


/*************************************************
* Name:        indcpa_dec
*
* Description: Decryption function of the CPA-secure
*              public-key encryption scheme underlying Kyber.
*
* Arguments:   - unsigned char *m:        pointer to output decrypted message (of length KYBER_INDCPA_MSGBYTES)
*              - const unsigned char *c:  pointer to input ciphertext (of length KYBER_INDCPA_BYTES)
*              - const unsigned char *sk: pointer to input secret key (of length KYBER_INDCPA_SECRETKEYBYTES)
**************************************************/
pub fn indcpa_dec(m: &mut[u8], c: &[u8], sk: &[u8])
{
  let (mut bp, mut skpv) = (Polyvec::new(),Polyvec::new());
  let (mut v, mut mp) = (Poly::new(),Poly::new());
 
  unpack_ciphertext(&mut bp, &mut v, c);
  unpack_sk(&skpv, sk);

  polyvec_ntt(&bp);
  polyvec_pointwise_acc(&mp, &skpv, &bp);
  poly_invntt(&mut mp);

  poly_sub(&mut mp, &v, &mp);
  poly_reduce(&mut mp);

  poly_tomsg(m, &mut mp);
}