<style>
	.card {
		/* color used to softly clip top and bottom of the .words container */
		--bg-color: #0d0e13;
		background-color: var(--bg-color);
		padding: 1rem 2rem;
		border-radius: 1.25rem;
	}
	.loader {
		color: rgb(124, 124, 124);
		font-family: "Poppins", sans-serif;
		font-weight: 500;
		font-size: 25px;
		-webkit-box-sizing: content-box;
		box-sizing: content-box;
		height: 40px;
		padding: 10px 10px;
		display: -webkit-box;
		display: -ms-flexbox;
		display: flex;
		border-radius: 8px;
	}

	.words {
		overflow: hidden;
	}

	.word {
		display: block;
		height: 100%;
		padding-left: 6px;
		color: #a6b0bb;
		animation: spin_4991 4s 1;
	}

	@keyframes spin_4991 {
		10% {
			-webkit-transform: translateY(-102%);
			transform: translateY(-102%);
		}

		25% {
			-webkit-transform: translateY(-100%);
			transform: translateY(-100%);
		}

		35% {
			-webkit-transform: translateY(-202%);
			transform: translateY(-202%);
		}

		50% {
			-webkit-transform: translateY(-200%);
			transform: translateY(-200%);
		}

		60% {
			-webkit-transform: translateY(-302%);
			transform: translateY(-302%);
		}

		75% {
			-webkit-transform: translateY(-300%);
			transform: translateY(-300%);
		}

		85% {
			-webkit-transform: translateY(-402%);
			transform: translateY(-402%);
		}

		100% {
			-webkit-transform: translateY(-400%);
			transform: translateY(-400%);
		}
	}
</style>

<div class="card">
	<div class="loader">
		<div class="words">
			<span class="word text-center"
				>Cryptographic operations in progress...</span
			>
			<span class="word text-center">Fetching public keys...</span>
			<span class="word text-center">Invoking WASM module...</span>
			<span class="word text-center">Encrypting Credentials...</span>
			<span class="word text-center">Adding Digital Signature...</span>
		</div>
	</div>
</div>
