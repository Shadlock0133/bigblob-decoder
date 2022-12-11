// int CMP_CDECL CompressBlockBC7(
//      const unsigned char* srcBlock,
//      unsigned int srcStrideInBytes, 
//      unsigned char cmpBlock[16],
//      const void* options CMP_DEFAULTNULL);
int CompressBlockBC7(
    const unsigned char* srcBlock,
    unsigned int srcStrideInBytes, 
    unsigned char cmpBlock[16],
    const void* options);
